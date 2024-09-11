use anyhow::{anyhow, Context};
use axum_prometheus::{
    metrics::{counter, histogram},
    metrics_exporter_prometheus::PrometheusBuilder,
};
use backoff::{future::retry, ExponentialBackoff};
use clap::Parser;
use concordium_rust_sdk::{
    types::AbsoluteBlockHeight,
    v2::{Client, Endpoint, FinalizedBlockInfo, FinalizedBlocksStream},
};
use dotenv::dotenv;
use gcp_auth::CustomServiceAccount;
use log::{debug, error, info};
use notification_server::{
    database,
    database::DatabaseConnection,
    google_cloud::{GoogleCloud, NotificationError::UnregisteredError},
    processor::process,
};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tonic::{codegen::http, transport::ClientTlsConfig};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoint is expected to point to a concordium node grpc v2 API's.",
        default_value = "https://grpc.testnet.concordium.com:20000",
        env = "NOTIFICATION_SERVER_BACKEND_NODE"
    )]
    endpoint: Endpoint,
    /// Database connection string.
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "NOTIFICATION_SERVER_DB_CONNECTION"
    )]
    db_connection: tokio_postgres::config::Config,
    #[arg(
        long = "google-application-credentials",
        help = "Credentials used for permitting the application to send push notifications.",
        env = "NOTIFICATION_SERVER_GOOGLE_APPLICATION_CREDENTIALS_PATH"
    )]
    google_application_credentials_path: String,
    #[arg(
        long = "google-client-timeout-secs",
        help = "Request timeout connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_TIMEOUT_SECS",
        default_value_t = 30
    )]
    google_client_timeout_secs: u64,
    #[arg(
        long = "google-client-connection-timeout-secs",
        help = "Request connection timeout connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_CONNECTION_TIMEOUT_SECS",
        default_value_t = 5
    )]
    google_client_connection_timeout_secs: u64,
    #[arg(
        long = "google-client-max-elapsed-time-secs",
        help = "Max elapsed time for connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_MAX_ELAPSED_TIME_SECS",
        default_value_t = 900  // 15 minutes
    )]
    google_client_max_elapsed_time_secs: u64,
    #[arg(
        long = "google-client-max-interval-time-secs",
        help = "Max interval time for retries when connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_MAX_INTERVAL_TIME_SECS",
        default_value_t = 180  // 3 minutes
    )]
    google_client_max_interval_time_secs: u64,
    #[arg(
        long = "log-level",
        default_value = "info",
        help = "The maximum log level. Possible values are: `trace`, `debug`, `info`, `warn`, and \
                `error`.",
        env = "LOG_LEVEL"
    )]
    log_level: tracing_subscriber::filter::LevelFilter,
    #[arg(
        long = "process-timeout-secs",
        default_value_t = 60,
        help = "Specifies in seconds the maximum wait for the next block to be processed.",
        env = "NOTIFICATION_SERVER_PROCESS_TIMEOUT_SEC"
    )]
    block_process_timeout_sec: u64,
    #[arg(
        long = "listen-address",
        help = "Listen address for the server.",
        env = "NOTIFICATION_SERVER_PROMETHEUS_ADDRESS"
    )]
    listen_address: Option<std::net::SocketAddr>,
}

const DATABASE_RETRY_DELAY: Duration = Duration::from_secs(1);

async fn process_block(
    database_connection: &DatabaseConnection,
    gcloud: &GoogleCloud<CustomServiceAccount>,
    concordium_client: &mut Client,
    finalized_block: FinalizedBlockInfo,
) -> anyhow::Result<AbsoluteBlockHeight> {
    info!(
        "Processed block {} at height {}",
        finalized_block.block_hash, finalized_block.height
    );
    let block_hash = finalized_block.block_hash;
    let operation = || async {
        concordium_client
            .clone()
            .get_block_transaction_events(block_hash)
            .await
            .map_err(|err| {
                error!(
                    "Error occurred while trying to read transactions: {:?}",
                    err
                );
                backoff::Error::transient(err)
            })
    };

    let transactions = match retry(
        ExponentialBackoff {
            max_elapsed_time: Some(Duration::from_secs(5)),
            ..ExponentialBackoff::default()
        },
        operation,
    )
    .await
    {
        Ok(transactions) => transactions.response,
        Err(err) => {
            return Err(anyhow!(
                "Error occurred while reading transactions: {}. Proceeding",
                err
            ));
        }
    };

    for result in process(transactions).await.into_iter() {
        debug!(
            "Sending notifications to account {} with type {:?}",
            result.address(),
            result.transaction_type()
        );
        let operation = || async {
            match database_connection
                .prepared
                .get_devices_from_account(result.address())
                .await
            {
                Ok(devices) => Ok(devices),
                Err(err) => {
                    error!(
                        "Error retrieving devices for account {}: {:?}. Retrying...",
                        result.address(),
                        err
                    );
                    Err(backoff::Error::transient(err))
                }
            }
        };

        let devices = retry(
            backoff::backoff::Constant::new(DATABASE_RETRY_DELAY),
            operation,
        )
        .await
        .unwrap_or_else(|_| {
            error!("Error occurred while reading devices. We should never be here");
            Vec::new()
        });

        let devices: Vec<_> = devices
            .iter()
            .filter(|device| device.preferences.contains(result.transaction_type()))
            .collect();

        if devices.is_empty() {
            debug!(
                "No devices subscribed to account {} having preference {:?}",
                result.address(),
                result.transaction_type()
            );
            continue;
        }

        info!(
            "Sending notification to {} devices for address {}",
            devices.len(),
            result.address()
        );
        let enriched_notification_information =
            match result.enrich(concordium_client.clone(), block_hash).await {
                Ok(information) => information,
                Err(err) => {
                    return Err(anyhow!(
                        "Error occurred while enriching notification information: {}. Proceeding",
                        err
                    ));
                }
            };
        for device in devices {
            match gcloud
                .send_push_notification(&device.device_token, &enriched_notification_information)
                .await
            {
                Ok(_) => counter!("notification.send_total").increment(1),
                Err(err) => {
                    if err == UnregisteredError {
                        info!("Device {} is unregistered", device.device_token);
                        counter!("notification.send_unregistered").increment(1);
                    } else {
                        error!("Error occurred while sending notification: {:?}", err);
                        counter!("notification.send_error").increment(1);
                    }
                }
            }
        }
    }
    let operation = || async {
        database_connection
            .prepared
            .insert_block(&block_hash, &finalized_block.height)
            .await
            .map_err(|err| match err {
                database::Error::DatabaseConnection(_) | database::Error::PoolError(_) => {
                    error!("Error writing to database {:?}. Retrying...", err);
                    backoff::Error::transient(err)
                }
                database::Error::ConstraintViolation(_, _) => backoff::Error::permanent(err),
            })
    };

    if let Err(err) = retry(
        backoff::backoff::Constant::new(DATABASE_RETRY_DELAY),
        operation,
    )
    .await
    {
        return Err(anyhow!(
            "Error occurred while writing to database: {}. Proceeding",
            err
        ));
    };
    Ok(finalized_block.height)
}

/// This function continuously processes blocks received from a
/// `FinalizedBlocksStream`, retrieves the associated transactions, and sends
/// notifications based on those transactions. The process involves interacting
/// with a database, a Concordium client, and Google Cloud services.
///
/// # Arguments
///
/// - `database_connection`: A reference to the `DatabaseConnection` used for
///   database interactions.
/// - `concordium_client`: A mutable reference to the Concordium client used to
///   interact with the Concordium network.
/// - `gcloud`: A reference to the `GoogleCloud` client used for sending push
///   notifications.
/// - `receiver`: A stream of finalized blocks that need to be processed.
/// - `process_timeout`: A `Duration` that specifies the timeout for processing
///   each block.
/// - `height`: The starting block height from which processing begins.
///
/// # Returns
///
/// Returns the `AbsoluteBlockHeight` of the last processed block given error
/// occurs such that we can proceed from the next block when we try and
/// reestablish the stream.
///
/// # Error Handling
///
/// The function is designed to continue processing blocks even if errors occur
/// while handling specific transactions. The error handling strategy is as
/// follows:
///
/// - **Block Reading Errors**: If an error occurs while reading a block from
///   the stream, it logs the error and continues to the next block. This
///   ensures that the process does not halt due to issues in a specific block
/// - **Transaction Retrieval Errors**: If an error occurs while retrieving
///   transactions for a block, the error is logged, and the block is skipped.
///   The function will continue processing subsequent blocks.
/// - **Device Retrieval Errors**: When retrieving devices associated with a
///   transaction, the function uses an exponential backoff strategy to retry
///   the operation in case of transient errors. Permanent errors are logged,
///   and the operation continues with the next transaction.
/// - **Notification Sending Errors**: If sending a notification fails, the
///   function logs the error. If the error indicates that the device is
///   unregistered, it logs an informational message and continues. Other errors
///   are logged as more severe, but the processing of other notifications is
///   not interrupted.
/// - **Database Write Errors**: Similar to device retrieval, database write
///   errors are handled using a retry mechanism. Transient errors trigger a
///   retry, while permanent errors are logged, and the process continues.
async fn traverse_chain(
    database_connection: &DatabaseConnection,
    concordium_client: &mut Client,
    gcloud: &GoogleCloud<CustomServiceAccount>,
    mut receiver: FinalizedBlocksStream,
    process_timeout: Duration,
    mut processed_height: AbsoluteBlockHeight,
) -> AbsoluteBlockHeight {
    while let Some(v) = receiver.next_timeout(process_timeout).await.transpose() {
        let finalized_block = match v {
            Ok(v) => v,
            Err(e) => {
                error!("Error while reading block: {:?}", e);
                continue;
            }
        };
        let start = Instant::now();
        match process_block(
            database_connection,
            gcloud,
            concordium_client,
            finalized_block,
        )
        .await
        {
            Ok(block_height) => {
                processed_height = block_height;
                let delta = start.elapsed();
                histogram!("block.process_successful_duration").record(delta);
            }
            Err(err) => {
                error!("Error occurred while processing block: {:?}", err);
            }
        }
        counter!("block.process_total").increment(1);
    }
    processed_height
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();

    let log_filter = tracing_subscriber::filter::Targets::new()
        .with_target(module_path!(), args.log_level)
        .with_target("tower_http", args.log_level)
        .with_target("tokio_postgres", args.log_level);

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(log_filter)
        .init();

    if let Some(listen_address) = args.listen_address {
        let builder = PrometheusBuilder::new();
        builder
            .with_http_listener(listen_address)
            .install()
            .expect("failed to install metrics exporter");
    }

    let endpoint = if args
        .endpoint
        .uri()
        .scheme()
        .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
    {
        args.endpoint.tls_config(ClientTlsConfig::new())?
    } else {
        args.endpoint
    }
    .connect_timeout(Duration::from_secs(10))
    .timeout(Duration::from_secs(300))
    .http2_keep_alive_interval(Duration::from_secs(300))
    .keep_alive_timeout(Duration::from_secs(10))
    .keep_alive_while_idle(true);

    let retry_policy = ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(
            args.google_client_max_elapsed_time_secs,
        )),
        max_interval: Duration::from_secs(args.google_client_max_interval_time_secs),
        ..ExponentialBackoff::default()
    };

    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(
            args.google_client_connection_timeout_secs,
        ))
        .timeout(Duration::from_secs(args.google_client_timeout_secs))
        .build()?;

    let path = PathBuf::from(args.google_application_credentials_path);
    let service_account = CustomServiceAccount::from_file(path)?;
    let project_id = service_account
        .project_id()
        .context("Project ID not found in service account")?
        .to_string();
    let gcloud = GoogleCloud::new(http_client, retry_policy, service_account, &project_id);
    let database_connection = DatabaseConnection::create(args.db_connection).await?;
    let mut concordium_client = Client::new(endpoint).await?;
    let mut height = if let Some(height) = database_connection
        .prepared
        .get_processed_block_height()
        .await
        .context("Failed to get processed block height")?
    {
        height
    } else {
        concordium_client
            .get_consensus_info()
            .await?
            .last_finalized_block_height
    };
    loop {
        info!("Establishing stream of finalized blocks");
        let receiver = match concordium_client
            .get_finalized_blocks_from(height.next())
            .await
        {
            Ok(receiver) => receiver,
            Err(err) => {
                info!("Error occurred while reading finalized blocks: {:?}", err);
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };
        height = traverse_chain(
            &database_connection,
            &mut concordium_client,
            &gcloud,
            receiver,
            Duration::from_secs(args.block_process_timeout_sec),
            height,
        )
        .await;
    }
}

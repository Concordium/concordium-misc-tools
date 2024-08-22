use anyhow::Context;
use backoff::ExponentialBackoff;
use clap::Parser;
use concordium_rust_sdk::v2::{Client, Endpoint, FinalizedBlockInfo};
use dotenv::dotenv;
use futures::Stream;
use gcp_auth::CustomServiceAccount;
use log::{error, info};
use notification_server::{
    database::DatabaseConnection,
    google_cloud::{GoogleCloud, NotificationError},
    processor::process,
};
use std::{path::PathBuf, time::Duration};
use tokio::time::sleep;
use tonic::{
    codegen::{http, tokio_stream::StreamExt},
    transport::ClientTlsConfig,
};
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
    #[clap(
        long = "log-level",
        default_value = "info",
        help = "The maximum log level. Possible values are: `trace`, `debug`, `info`, `warn`, and \
                `error`.",
        env = "LOG_LEVEL"
    )]
    log_level: tracing_subscriber::filter::LevelFilter,
}

const DATABASE_RETRY_DELAY: Duration = Duration::from_secs(1);

async fn traverse_chain(
    database_connection: &DatabaseConnection,
    concordium_client: &mut Client,
    gcloud: &GoogleCloud<CustomServiceAccount>,
    mut receiver: impl Stream<Item = Result<FinalizedBlockInfo, tonic::Status>> + Unpin,
) {
    while let Some(v) = receiver.next().await {
        let finalized_block = match v {
            Ok(v) => v,
            Err(e) => {
                error!("Error while reading block: {:?}", e);
                continue;
            }
        };
        info!(
            "Processed block {} at height {}",
            finalized_block.block_hash, finalized_block.height
        );
        let block_hash = finalized_block.block_hash;
        let transactions = match concordium_client
            .get_block_transaction_events(block_hash)
            .await
        {
            Ok(transactions) => transactions.response,
            Err(err) => {
                error!("Error occurred while reading transactions: {:?}", err);
                continue;
            }
        };
        for result in process(transactions).await.iter() {
            info!(
                "Sending notification to account {} with type {:?}",
                result.address(),
                result.transaction_type()
            );
            let devices: Vec<_> = loop {
                match database_connection
                    .prepared
                    .get_devices_from_account(result.address().clone())
                    .await
                {
                    Ok(devices) => break devices,
                    Err(err) => {
                        error!(
                            "Error retrieving devices for account {}: {:?}. Retrying...",
                            result.address(),
                            err
                        );
                        sleep(DATABASE_RETRY_DELAY).await;
                    }
                }
            };
            let devices: Vec<_> = devices
                .iter()
                .filter(|device| device.preferences.contains(result.transaction_type()))
                .collect();
            if devices.is_empty() {
                info!(
                    "No devices subscribed to account {} having preference {:?}",
                    result.address(),
                    result.transaction_type()
                );
                continue;
            }
            let enriched_notification_information = match result
                .clone()
                .enrich(concordium_client.clone(), block_hash)
                .await
            {
                Ok(information) => information,
                Err(err) => {
                    error!(
                        "Error occurred while enriching notification information: {:?}",
                        err
                    );
                    continue;
                }
            };

            for device in devices {
                if let Err(err) = gcloud
                    .send_push_notification(
                        &device.device_token,
                        enriched_notification_information.clone(),
                    )
                    .await
                {
                    if err == NotificationError::UnregisteredError {
                        info!("Device {} is unregistered", device.device_token);
                    } else {
                        error!("Error occurred while sending notification: {:?}", err);
                    }
                }
            }
        }
    }
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

    loop {
        info!("Establishing stream of finalized blocks");
        let receiver = match concordium_client.get_finalized_blocks().await {
            Ok(receiver) => receiver,
            Err(err) => {
                info!("Error occurred while reading finalized blocks: {:?}", err);
                tokio::time::sleep(Duration::from_secs(10)).await;
                continue;
            }
        };
        traverse_chain(
            &database_connection,
            &mut concordium_client,
            &gcloud,
            receiver,
        )
        .await;
    }
}

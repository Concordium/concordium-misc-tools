use backoff::ExponentialBackoff;
use clap::Parser;
use concordium_rust_sdk::v2::{Client, Endpoint};
use dotenv::dotenv;
use notification_server::{
    database::DatabaseConnection, google_cloud::GoogleCloud, processor::process,
};
use std::{path::PathBuf, time::Duration};
use tonic::{
    codegen::{http, tokio_stream::StreamExt},
    transport::ClientTlsConfig,
};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoint is expected to point to a concordium node grpc v2 API's.",
        default_value = "https://grpc.testnet.concordium.com:20000"
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
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();
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

    let gcloud = GoogleCloud::new(
        PathBuf::from(args.google_application_credentials_path),
        http_client,
        retry_policy,
    )?;
    let database_connection = DatabaseConnection::create(args.db_connection).await?;

    let mut concordium_client = Client::new(endpoint).await?;
    let mut receiver = concordium_client.get_finalized_blocks().await?;
    while let Some(v) = receiver.next().await {
        let block_hash = v?.block_hash;
        println!("Blockhash: {:?}", block_hash);
        let transactions = concordium_client
            .get_block_transaction_events(block_hash)
            .await?
            .response;
        for result in process(transactions).await.iter() {
            println!("address: {}, amount: {}", result.address, result.amount);
            for device in database_connection
                .prepared
                .get_devices_from_account(&result.address.0)
                .await?
                .iter()
            {
                gcloud
                    .send_push_notification(device, result.to_owned())
                    .await?;
            }
        }
    }
    Ok(())
}

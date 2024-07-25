use clap::Parser;
use concordium_rust_sdk::v2::{Client, Endpoint};
use dotenv::dotenv;
use notification_server::{
    database::DatabaseConnection, google_cloud::GoogleCloud, processor::process,
};
use std::path::PathBuf;
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
        help = "",
        env = "NOTIFICATION_SERVER_GOOGLE_APPLICATION_CREDENTIALS_PATH"
    )]
    google_application_credentials_path: String,
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
    .connect_timeout(std::time::Duration::from_secs(10))
    .timeout(std::time::Duration::from_secs(300))
    .http2_keep_alive_interval(std::time::Duration::from_secs(300))
    .keep_alive_timeout(std::time::Duration::from_secs(10))
    .keep_alive_while_idle(true);

    let gcloud = GoogleCloud::new(PathBuf::from(args.google_application_credentials_path))?;
    let database_connection = DatabaseConnection::create(args.db_connection).await?;

    let mut client = Client::new(endpoint).await?;
    let mut receiver = client.get_finalized_blocks().await?;
    while let Some(v) = receiver.next().await {
        let block_hash = v?.block_hash;
        println!("Blockhash: {:?}", block_hash);
        let transactions = client
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

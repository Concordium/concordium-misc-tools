use clap::Parser;
use concordium_rust_sdk::v2::{Client, Endpoint};
use dotenv::dotenv;
use notification_server::processor::process;
use tonic::{
    codegen::{http, tokio_stream::StreamExt},
    transport::ClientTlsConfig,
};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoints are expected to point to concordium node grpc v2 API's.",
        default_value = "https://grpc.testnet.concordium.com:20000"
    )]
    endpoint:      Endpoint,
    /// Database connection string.
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "NOTIFICATION_SERVER_DB_CONNECTION"
    )]
    db_connection: tokio_postgres::config::Config,
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

    let mut client = Client::new(endpoint).await?;
    let mut receiver = client.get_finalized_blocks().await?;
    while let Some(v) = receiver.next().await {
        let block_hash = v?.block_hash;
        println!("Blockhash: {:?}", block_hash);
        let transactions = client
            .get_block_transaction_events(block_hash)
            .await?
            .response;
        let addresses = process(transactions).await;
        println!("Addresses: {:#?}", addresses);
    }
    Ok(())
}

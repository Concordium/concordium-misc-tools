use clap::Parser;
use concordium_rust_sdk::v2;
use thiserror::Error;

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "http://localhost:20001")]
    node: v2::Endpoint,
}

#[derive(Debug, Error)]
enum NodeError {
    /// Error establishing connection.
    #[error("Error connecting to the node {0}.")]
    ConnectionError(tonic::transport::Error),
}

async fn use_node(endpoint: v2::Endpoint) -> Result<(), NodeError> {
    println!("Using node {}", endpoint.uri());

    let node = v2::Client::new(endpoint)
        .await
        .map_err(NodeError::ConnectionError)?;

    println!("{:?}", node);

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = use_node(args.node).await {
        println!("Error happened: {}", error)
    }
}

use anyhow::{self, bail, Context};
use clap::Parser;
use concordium_rust_sdk::{common::Serial, endpoints::Endpoint, types::hashes::TransactionHash, v2};
use futures::stream::StreamExt;
use std::path::PathBuf;
use std::io::Write;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct App {
    #[arg(
        long = "node",
        help = "GRPC interface of the node.",
        default_value = "http://localhost:20000"
    )]
    endpoint:    Endpoint,
    #[arg(help = "transaction to fetch", long = "transaction", short = 't')]
    transaction: TransactionHash,
    #[arg(long = "out", short = 'o', help = "file to write transaction to")]
    output:      Option<PathBuf>,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();
    let mut client = {
        let endpoint = app.endpoint;
        let ep = endpoint.connect_timeout(std::time::Duration::from_secs(10));
        v2::Client::new(ep)
            .await
            .context("Unable to connect to node")?
    };
    let status = client
        .get_block_item_status(&app.transaction)
        .await
        .context("Unable to get transaction status")?;
    if let Some((block_hash, status)) = status.is_finalized() {
        let block_items = client
            .get_block_items(&block_hash)
            .await
            .context("Unable to get block items")?;

        // Get the transaction from the block items.
        // It is at status.index in the block items Stream.
        let transaction = block_items
            .response
            .skip(status.index.index as usize)
            .next()
            .await
            .ok_or(anyhow::anyhow!("Stream ended before transaction"))??;
        // Serialize the transaction as bytes to the output file or stdout.
        let mut buffer = Vec::new();
        transaction.serial(&mut buffer);
        if let Some(path) = app.output {
            std::fs::write(path, buffer)
                .context("Unable to write transaction to file")?;
        } else {
            let stdout = std::io::stdout();
            let mut stdout = stdout.lock();
            stdout.write_all(&buffer)
                .context("Unable to write transaction to stdout")?;
        }
    } else {
        bail!("Transaction is not finalized");
    }
    Ok(())
}

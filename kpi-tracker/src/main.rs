use anyhow::Context;
use clap::Parser;
use concordium_rust_sdk::{
    types::AbsoluteBlockHeight,
    v2::{self, FinalizedBlockInfo},
};

#[derive(Debug, Parser)]
struct Args {
    #[arg(long, default_value = "http://localhost:20001")]
    node: v2::Endpoint,
}

fn handle_block(block: FinalizedBlockInfo) {
    println!("Block: {:?}", block);
}

async fn use_node(endpoint: v2::Endpoint, height: u64) -> anyhow::Result<()> {
    println!("Using node {}\n", endpoint.uri());

    let mut node = v2::Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    // 1. Traverse all blocks (try Client.get_finalized_blocks_from)

    let mut blocks_stream = node
        .get_finalized_blocks_from(AbsoluteBlockHeight { height })
        .await
        .context("What happened here??")?;

    for _ in 0..11 {
        if let Some(block) = blocks_stream.next().await {
            handle_block(block);
        } else {
            println!("None");
        }
    }

    // 2. Find find all transactions in block (try Client.get_all_transaction_events,
    //    Client.get_all_special_events)
    // 3. Log accounts created in block with timestamp

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = use_node(args.node, 0).await {
        println!("Error happened: {}", error)
    }
}

use std::collections::{BTreeMap, HashMap};

use anyhow::Context;
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    types::{
        hashes::{BlockMarker, HashBytes},
        AbsoluteBlockHeight,
    },
    v2::{self, AccountIdentifier, FinalizedBlockInfo},
};
use futures::{self, StreamExt};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(long, default_value = "http://localhost:20001")]
    node: v2::Endpoint,
    /// How many blocks to process.
    // Only here for testing purposes...
    #[arg(long, default_value_t = 10000)]
    num_blocks: u64,
}

#[derive(Debug)]
struct AccountDetails {
    is_initial: bool,
    block_hash: String, // FK to blocks
}

// Blocks are stored, so other tables can reference information about the block they were created in.
#[derive(Debug)]
struct BlockDetails {
    block_time: i64,
}

type AccountsTable = HashMap<String, AccountDetails>;
type BlocksTable = HashMap<String, BlockDetails>;

struct DB {
    blocks: BlocksTable,
    accounts: AccountsTable,
}

/// Returns a Map of AccountAddress, AccountDetails pairs not already included in `accounts_table`
async fn new_accounts_in_block(
    node: &mut v2::Client,
    block_hash: &HashBytes<BlockMarker>,
    accounts_table: &AccountsTable,
) -> anyhow::Result<BTreeMap<String, AccountDetails>> {
    let mut accounts = node
        .get_account_list(block_hash)
        .await
        .context(format!("Could not get accounts for block: {}", block_hash))?
        .response;

    let mut new_accounts = BTreeMap::new();

    while let Some(res) = accounts.next().await {
        let account = res.context("What exactly is this status error?")?; // TODO
        let key = account.to_string();

        if !accounts_table.contains_key(&key) {
            // Client needs to be cloned for it to not be consumed on the first run.
            let mut c_node = node.clone();

            let account_info = c_node
                .get_account_info(&AccountIdentifier::Address(account), block_hash)
                .await?
                .response;

            let is_initial = account_info
                .account_credentials
                .get(&0.into())
                .map_or(false, |cdi| match &cdi.value {
                    AccountCredentialWithoutProofs::Initial { .. } => true,
                    AccountCredentialWithoutProofs::Normal { .. } => false,
                });

            let account_details = AccountDetails {
                is_initial,
                block_hash: block_hash.to_string(),
            };

            new_accounts.insert(account.to_string(), account_details);

            println!(
                "NEW ACCOUNT:\naccount: {}\ndetails: {:?}",
                &key,
                new_accounts.get(&account.to_string())
            );
        }
    }

    Ok(new_accounts)
}

/// Insert as a single DB transaction to facilitate easy recovery, as the service can restart from
/// current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (&HashBytes<BlockMarker>, BlockDetails),
    accounts: BTreeMap<String, AccountDetails>,
) {
    db.blocks.insert(block_hash.to_string(), block_details);
    db.accounts.extend(accounts.into_iter());
}

async fn handle_block(
    node: &mut v2::Client,
    block: FinalizedBlockInfo,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block.block_hash)
        .await
        .context(format!(
            "Could not get block info for block: {}",
            block.block_hash
        ))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time.timestamp(),
    };
    let new_accounts = new_accounts_in_block(node, &block_info.block_hash, &db.accounts).await?;

    update_db(db, (&block_info.block_hash, block_details), new_accounts);

    Ok(())
}

async fn use_node(endpoint: v2::Endpoint, height: u64) -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Using node {}\n", endpoint.uri());

    let mut db = DB {
        blocks: HashMap::new(),
        accounts: HashMap::new(),
    };

    let mut node = v2::Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(AbsoluteBlockHeight { height })
        .await
        .context("Error querying blocks")?;

    for _ in 0..args.num_blocks {
        if let Some(block) = blocks_stream.next().await {
            handle_block(&mut node, block, &mut db).await?;
        }
    }

    println!("\n");
    println!("Blocks stored: {}\n", &db.blocks.len());

    let accounts: Vec<String> = db
        .accounts
        .into_iter()
        .map(|r| format!("{}, {:?}", r.0, r.1))
        .collect();
    println!("Accounts stored:\n {}", accounts.join("\n"));

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    if let Err(error) = use_node(args.node, 0).await {
        println!("Error happened: {}", error)
    }
}

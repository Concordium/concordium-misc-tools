use std::collections::{BTreeMap, HashMap};

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::AccountAddress,
    types::{
        hashes::{BlockHash, BlockMarker, HashBytes},
        AbsoluteBlockHeight,
        BlockItemSummaryDetails::AccountCreation,
    },
    v2::{self, AccountIdentifier, Client, Endpoint, FinalizedBlockInfo},
};
use futures::{self, StreamExt, TryStreamExt};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoint is expected to point to a concordium node grpc v2 API.",
        default_value = "http://localhost:20001"
    )]
    node: Endpoint,
    /// How many blocks to process.
    // Only here for testing purposes...
    #[arg(long = "num-blocks", default_value_t = 10000)]
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
    block_time: DateTime<Utc>,
    height: AbsoluteBlockHeight, // Used as a reference from where to restart on service restart.
}

type AccountsTable = HashMap<AccountAddress, AccountDetails>;
type BlocksTable = HashMap<String, BlockDetails>;

/// This is intended as a in-memory DB, which follows the same schema as the final DB will follow.
struct DB {
    blocks: BlocksTable,
    accounts: AccountsTable,
}

async fn account_details(
    node: &mut Client,
    block_hash: &BlockHash,
    account: &AccountAddress,
) -> anyhow::Result<AccountDetails> {
    let account_info = node
        .get_account_info(&AccountIdentifier::Address(*account), block_hash)
        .await?
        .response;

    let is_initial =
        account_info
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

    Ok(account_details)
}
async fn accounts_in_block(
    node: &mut Client,
    block_hash: &BlockHash,
) -> anyhow::Result<BTreeMap<AccountAddress, AccountDetails>> {
    let accounts = node
        .get_account_list(block_hash)
        .await
        .context(format!("Could not get accounts for block: {}", block_hash))?
        .response;

    let accounts_details_map = accounts
        .then(|res| {
            let mut node = node.clone();

            async move {
                let account = res?;
                let details = account_details(&mut node, block_hash, &account).await?;

                Ok((account, details))
            }
        })
        .try_fold(BTreeMap::new(), |mut map, (account, details)| async move {
            map.insert(account, details);
            Ok(map)
        })
        .await;

    accounts_details_map
}

/// Returns a Map of AccountAddress, AccountDetails pairs not already included in `accounts_table`
async fn new_accounts_in_block(
    node: &mut Client,
    block_hash: &BlockHash,
    accounts_table: &AccountsTable,
) -> anyhow::Result<BTreeMap<AccountAddress, AccountDetails>> {
    let mut transactions = node
        .get_block_transaction_events(block_hash)
        .await
        .context(format!(
            "Could not get transactions for block: {}",
            block_hash
        ))?
        .response;

    let mut new_accounts = BTreeMap::new();

    while let Some(res) = transactions.next().await {
        let transaction = res.context("Stream stopped prematurely")?;

        match transaction.details {
            AccountCreation(act)
                if !accounts_table
                    .keys()
                    .into_iter()
                    .any(|stored_address| act.address.is_alias(stored_address)) =>
            {
                let address = act.address;
                let account_details = account_details(node, block_hash, &address).await?;

                new_accounts.insert(address, account_details);
            }
            _ => {}
        }
    }

    Ok(new_accounts)
}

/// Insert as a single DB transaction to facilitate easy recovery, as the service can restart from
/// current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (&HashBytes<BlockMarker>, BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
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
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
    };

    let new_accounts = if block.height.height == 0 {
        accounts_in_block(node, &block.block_hash).await?
    } else {
        new_accounts_in_block(node, &block_info.block_hash, &db.accounts).await?
    };

    update_db(db, (&block_info.block_hash, block_details), new_accounts);

    Ok(())
}

async fn use_node(endpoint: v2::Endpoint, height: AbsoluteBlockHeight) -> anyhow::Result<()> {
    let args = Args::parse();

    println!("Using node {}\n", endpoint.uri());

    let mut db = DB {
        blocks: HashMap::new(),
        accounts: HashMap::new(),
    };

    let mut node = Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(height)
        .await
        .context("Error querying blocks")?;

    for _ in 0..args.num_blocks {
        if let Some(block) = blocks_stream.next().await {
            handle_block(&mut node, block, &mut db).await?;
        }
    }

    println!("\n");
    println!("Blocks stored: {}\n", &db.blocks.len());

    let mut accounts: Vec<(AccountAddress, DateTime<Utc>, AccountDetails)> = db
        .accounts
        .into_iter()
        .map(|(address, details)| {
            let block_time = db
                .blocks
                .get(&details.block_hash)
                .expect("Found account with wrong reference to block?")
                .block_time;

            (address, block_time, details)
        })
        .collect();

    accounts.sort_by_cached_key(|v| v.1);

    let account_strings: Vec<String> = accounts
        .into_iter()
        .map(|(address, block_time, details)| format!("{}, {}, {:?}", address, block_time, details))
        .collect();

    println!("Accounts stored:\n{}", account_strings.join("\n"));

    Ok(())
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    let height = AbsoluteBlockHeight { height: 0 };
    if let Err(error) = use_node(args.node, height).await {
        println!("Error happened: {}", error)
    }
}

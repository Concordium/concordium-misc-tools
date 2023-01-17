use std::collections::{BTreeMap, HashMap};

use anyhow::Context;
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::AccountAddress,
    types::{
        hashes::BlockHash, AbsoluteBlockHeight, AccountCreationDetails,
        BlockItemSummaryDetails::AccountCreation, CredentialType,
    },
    v2::{self, AccountIdentifier, Client, Endpoint},
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

/// Holds information about accounts created on chain.
#[derive(Debug)]
struct AccountDetails {
    /// Whether an account was created as an initial account or not.
    is_initial: bool,
    /// Link to the block in which the account was created.
    block_hash: BlockHash,
}

/// Information about individual blocks. Useful for linking entities to a block and it's
/// corresponding attributes.
#[derive(Debug, Clone, Copy)]
struct BlockDetails {
    /// Finalization time of the block. Used to show how metrics evolve over time by linking entities, such as accounts and transactions, to
    /// the block in which they are created.
    block_time: DateTime<Utc>,
    /// Height of block from genesis. Used to restart the process of collecting metrics from the
    /// latest block recorded.
    height: AbsoluteBlockHeight,
}

type AccountsTable = HashMap<AccountAddress, AccountDetails>;
type BlocksTable = HashMap<BlockHash, BlockDetails>;

/// This is intended as a in-memory DB, which follows the same schema as the final DB will follow.
struct DB {
    /// Table containing all blocks queried from node.
    blocks: BlocksTable,
    /// Table containing all accounts created on chain, along with accounts present in genesis
    /// block
    accounts: AccountsTable,
}

/// Queries node for account info for the `account` given at the block represented by the
/// `block_hash`
fn account_details(
    block_hash: BlockHash,
    account_creation_details: Option<AccountCreationDetails>,
) -> AccountDetails {
    let is_initial = account_creation_details.map_or(false, |act| match act.credential_type {
        CredentialType::Initial { .. } => true,
        CredentialType::Normal { .. } => false,
    });

    AccountDetails {
        is_initial,
        block_hash,
    }
}

/// Returns accounts on chain at the give `block_hash`
async fn accounts_in_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<BTreeMap<AccountAddress, AccountDetails>> {
    let accounts = node
        .get_account_list(block_hash)
        .await
        .with_context(|| format!("Could not get accounts for block: {}", block_hash))?
        .response;

    let accounts_details_map = accounts
        .then(|res| {
            let mut node = node.clone();

            async move {
                let account = res.with_context(|| {
                    format!("Error while streaming accounts in block {}", block_hash)
                })?;
                let account_info = node
                    .get_account_info(&AccountIdentifier::Address(account), block_hash)
                    .await
                    .with_context(|| {
                        format!(
                            "Error while getting account info for account {} at block {}",
                            account, block_hash
                        )
                    })?
                    .response;

                anyhow::Ok((account, account_info))
            }
        })
        .try_fold(BTreeMap::new(), |mut map, (account, info)| async move {
            let is_initial = info
                .account_credentials
                .get(&0.into())
                .map_or(false, |cdi| match cdi.value {
                    AccountCredentialWithoutProofs::Initial { .. } => true,
                    AccountCredentialWithoutProofs::Normal { .. } => false,
                });

            let details = AccountDetails {
                is_initial,
                block_hash,
            };
            map.insert(account, details);

            Ok(map)
        })
        .await?;

    Ok(accounts_details_map)
}

/// Returns a Map of AccountAddress, AccountDetails created in the block represented by `block_hash`
async fn new_accounts_in_block(
    node: &mut Client,
    block_hash: BlockHash,
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
        let transaction = res.with_context(|| {
            format!(
                "Error while streaming transactions for block {}",
                block_hash
            )
        })?;

        match transaction.details {
            AccountCreation(act) => {
                let address = act.address;
                let account_details = account_details(block_hash, Some(act));

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
    (block_hash, block_details): (BlockHash, &BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
) {
    db.blocks.insert(block_hash, *block_details);
    db.accounts.extend(accounts.into_iter());
}

/// Processes a block, represented by `block_hash` by querying the node for entities present in the block state. Should only be
/// used to process the genesis block.
async fn process_genesis_block(
    node: &mut v2::Client,
    block_hash: BlockHash,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .context(format!(
            "Could not get block info for genesis block: {}",
            block_hash
        ))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
    };

    let genesis_accounts = accounts_in_block(node, block_hash).await?;
    update_db(db, (block_hash, &block_details), genesis_accounts);

    Ok(())
}

/// Process a block, represented by `block_hash`, updating the `db` corresponding to events captured by the block.
async fn process_block(
    node: &mut v2::Client,
    block_hash: BlockHash,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .context(format!(
            "Could not get block info for block: {}",
            block_hash
        ))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
    };

    let new_accounts = new_accounts_in_block(node, block_hash).await?;
    update_db(db, (block_hash, &block_details), new_accounts);

    Ok(())
}

/// Queries the node available at `endpoint` from `height` for `Args.num_blocks` blocks. Inserts
/// results captured into a `DB` and prints the result.
async fn use_node(endpoint: v2::Endpoint, height: AbsoluteBlockHeight) -> anyhow::Result<()> {
    let args = Args::parse();
    let blocks_to_process = height.height + args.num_blocks;

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

    if height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            process_genesis_block(&mut node, genesis_block.block_hash, &mut db).await?;
        }
    }

    for _ in height.height + 1..blocks_to_process {
        if let Some(block) = blocks_stream.next().await {
            process_block(&mut node, block.block_hash, &mut db).await?;
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

    accounts.sort_by_key(|v| v.1);

    let account_strings: Vec<String> = accounts
        .into_iter()
        .map(|(address, block_time, details)| format!("{}, {}, {:?}", address, block_time, details))
        .collect();

    println!("Accounts stored:\n{}", account_strings.join("\n"));

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let height = AbsoluteBlockHeight { height: 0 };

    use_node(args.node, height)
        .await
        .context("Error happened while querying node.")?;

    Ok(())
}

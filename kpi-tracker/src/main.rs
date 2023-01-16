use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::types::hashes::TransactionHash;
use concordium_rust_sdk::types::{
    AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary, ContractAddress,
    TransactionType,
};
use concordium_rust_sdk::{
    smart_contracts::common::AccountAddress,
    types::{
        hashes::BlockHash,
        AbsoluteBlockHeight, AccountCreationDetails,
        BlockItemSummaryDetails::{AccountCreation, AccountTransaction},
        CredentialType,
    },
    v2::{Client, Endpoint},
};
use futures::{self, future, Stream, StreamExt, TryStreamExt};

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

/// Holds information about accounts created on chain.
#[derive(Debug)]
struct AccountDetails {
    /// Whether an account was created as an initial account or not.
    is_initial: bool,
    /// Link to the block in which the account was created.
    block_hash: BlockHash,
}

// TODO: doc
#[derive(Debug)]
struct TransactionDetails {
    transaction_type: TransactionType,
    block_hash: BlockHash, // FK to blocks
    cost: u64,
}

// TODO: doc
#[derive(Debug)]
struct ContractModuleDetails {
    block_hash: BlockHash, // FK to transactions
}

// TODO: doc
#[derive(Debug)]
struct ContractInstanceDetails {
    module_ref: String,    // FK to modules
    block_hash: BlockHash, // FK to transactions
}

type BlocksTable = HashMap<BlockHash, BlockDetails>;
type AccountsTable = HashMap<AccountAddress, AccountDetails>;
type AccountTransactionsTable = HashMap<TransactionHash, TransactionDetails>;
type ContractModulesTable = HashMap<String, ContractModuleDetails>;
type ContractInstancesTable = HashMap<ContractAddress, ContractInstanceDetails>;

/// This is intended as a in-memory DB, which follows the same schema as the final DB will follow.
struct DB {
    /// Table containing all blocks queried from node.
    blocks: BlocksTable,
    /// Table containing all accounts created on chain, along with accounts present in genesis
    /// block
    accounts: AccountsTable,
    // TODO: doc
    account_transactions: AccountTransactionsTable,
    // TODO: doc
    contract_modules: ContractModulesTable,
    // TODO: doc
    contract_instances: ContractInstancesTable,
}

// TODO: doc
enum BlockEvent {
    AccountCreation(AccountAddress, AccountDetails),
    AccountTransaction(TransactionHash, TransactionDetails),
    ContractModuleDeployment(String, ContractModuleDetails),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
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
        .context(format!("Block not found: {}", block_hash))?
        .response;

    let accounts_details_map = accounts
        .map_ok(|account| {
            let details = account_details(block_hash, None);
            (account, details)
        })
        .try_fold(BTreeMap::new(), |mut map, (account, details)| async move {
            map.insert(account, details);
            Ok(map)
        })
        .await
        .context("Stream stopped prematurely")?;

    Ok(accounts_details_map)
}

fn get_account_transaction_details(
    details: &AccountTransactionDetails,
    block_hash: BlockHash,
) -> Option<TransactionDetails> {
    details
        .transaction_type()
        .map(|transaction_type| TransactionDetails {
            block_hash,
            transaction_type,
            cost: details.cost.micro_ccd,
        })
}

/// Maps `BlockItemSummary` to `BlockEvent`, which represent entities stored in the database.
fn to_block_events(block_hash: BlockHash, block_item: BlockItemSummary) -> Vec<BlockEvent> {
    let mut events: Vec<BlockEvent> = Vec::new();

    match block_item.details {
        AccountTransaction(atd) => {
            // TODO: do we want to store failed transactions?
            if let Some(details) = get_account_transaction_details(&atd, block_hash) {
                println!(
                    "TRANSACTION:\nhash: {}\ndetails: {:?}",
                    &block_item.hash, &details
                );
                let event = BlockEvent::AccountTransaction(block_item.hash, details);
                events.push(event);
            }

            match atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let details = ContractModuleDetails { block_hash };
                    println!(
                        "CONTRACT MODULE:\nref: {}\ndetails: {:?}",
                        &module_ref, &details
                    );
                    let event =
                        BlockEvent::ContractModuleDeployment(module_ref.to_string(), details);
                    events.push(event);
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        block_hash,
                        module_ref: data.origin_ref.to_string(),
                    };
                    println!(
                        "CONTRACT INSTANCE:\naddress: {}\ndetails: {:?}",
                        &data.address, &details
                    );
                    let event = BlockEvent::ContractInstantiation(data.address, details);
                    events.push(event);
                }
                _ => {}
            };
        }
        AccountCreation(act) => {
            let address = act.address;
            let details = account_details(block_hash, Some(act));

            println!("ACCOUNT:\naddress: {}\ndetails: {:?}", &address, &details);

            let block_event = BlockEvent::AccountCreation(address, details);
            events.push(block_event);
        }
        _ => {}
    };

    events
}

// Don't know why I need explicit lifetime anotations here?
fn process_transactions<'a>(
    block_hash: &'a BlockHash,
    transactions_stream: impl Stream<Item = Result<BlockItemSummary, tonic::Status>> + 'a,
) -> impl Stream<Item = anyhow::Result<BlockEvent>> + 'a {
    let block_events_stream = transactions_stream.flat_map(|res| {
        let block_events: Vec<Result<BlockEvent, anyhow::Error>> = match res {
            Ok(bi) => to_block_events(*block_hash, bi)
                .into_iter()
                .map(Ok)
                .collect(),
            Err(err) => vec![Err(anyhow!("Error: {}", err))],
        };

        futures::stream::iter(block_events)
    });

    block_events_stream
}

/// Insert as a single DB transaction to facilitate easy recovery, as the service can restart from
/// current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (BlockHash, &BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
    transactions: Option<BTreeMap<TransactionHash, TransactionDetails>>,
    contract_modules: Option<BTreeMap<String, ContractModuleDetails>>,
    contract_instances: Option<BTreeMap<ContractAddress, ContractInstanceDetails>>,
) {
    db.blocks.insert(block_hash, *block_details);
    db.accounts.extend(accounts.into_iter());
    if let Some(ts) = transactions {
        db.account_transactions.extend(ts.into_iter());
    }
    if let Some(ms) = contract_modules {
        db.contract_modules.extend(ms.into_iter());
    }
    if let Some(is) = contract_instances {
        db.contract_instances.extend(is.into_iter());
    }
}

/// Processes a block, represented by `block_hash` by querying the node for entities present in the block state. Should only be
/// used to process the genesis block.
async fn process_genesis_block(
    node: &mut Client,
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
    update_db(
        db,
        (block_hash, &block_details),
        genesis_accounts,
        None,
        None,
        None,
    );

    Ok(())
}

/// Process a block, represented by `block_hash`, updating the `db` corresponding to events captured by the block.
async fn process_block(
    node: &mut Client,
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

    let transactions_stream = node
        .get_block_transaction_events(block_info.block_hash)
        .await
        .context(format!("Block not found: {}", block_info.block_hash))?
        .response;

    let mut accounts: BTreeMap<AccountAddress, AccountDetails> = BTreeMap::new();
    let mut account_transactions: BTreeMap<TransactionHash, TransactionDetails> = BTreeMap::new();
    let mut contract_modules: BTreeMap<String, ContractModuleDetails> = BTreeMap::new();
    let mut contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails> =
        BTreeMap::new();

    let block_events = process_transactions(&block_info.block_hash, transactions_stream);

    block_events
        .try_for_each(|be| {
            match be {
                BlockEvent::AccountCreation(address, details) => {
                    accounts.insert(address, details);
                }
                BlockEvent::AccountTransaction(hash, details) => {
                    account_transactions.insert(hash, details);
                }
                BlockEvent::ContractModuleDeployment(module_ref, details) => {
                    contract_modules.insert(module_ref, details);
                }
                BlockEvent::ContractInstantiation(address, details) => {
                    contract_instances.insert(address, details);
                }
            };

            future::ok(())
        })
        .await?;

    update_db(
        db,
        (block_hash, &block_details),
        accounts,
        Some(account_transactions),
        Some(contract_modules),
        Some(contract_instances),
    );

    Ok(())
}

fn print_db(db: DB) {
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
        .map(|(address, block_time, details)| {
            format!("Account: {}, {}, {:?}", address, block_time, details)
        })
        .collect();
    println!("Accounts stored:\n{}\n", account_strings.join("\n"));

    let mut transactions: Vec<(TransactionHash, DateTime<Utc>, TransactionDetails)> = db
        .account_transactions
        .into_iter()
        .map(|(hash, details)| {
            let block_time = db
                .blocks
                .get(&details.block_hash)
                .expect("Found account with wrong reference to block?")
                .block_time;

            (hash, block_time, details)
        })
        .collect();

    transactions.sort_by_key(|v| v.1);

    let transaction_strings: Vec<String> = transactions
        .into_iter()
        .map(|(hash, block_time, details)| {
            format!("Transaction: {}, {}, {:?}", hash, block_time, details)
        })
        .collect();
    println!("Transactions stored:\n{}\n", transaction_strings.join("\n"));

    let mut contract_modules: Vec<(String, DateTime<Utc>, ContractModuleDetails)> = db
        .contract_modules
        .into_iter()
        .map(|(m_ref, details)| {
            let block_time = db
                .blocks
                .get(&details.block_hash)
                .expect("Found account with wrong reference to block?")
                .block_time;

            (m_ref, block_time, details)
        })
        .collect();

    contract_modules.sort_by_key(|v| v.1);

    let module_strings: Vec<String> = contract_modules
        .into_iter()
        .map(|(m_ref, block_time, details)| {
            format!("Transaction: {}, {}, {:?}", m_ref, block_time, details)
        })
        .collect();
    println!("Contract modules stored:\n{}\n", module_strings.join("\n"));

    let mut contract_instances: Vec<(ContractAddress, DateTime<Utc>, ContractInstanceDetails)> = db
        .contract_instances
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

    contract_instances.sort_by_key(|v| v.1);

    let instance_strings: Vec<String> = contract_instances
        .into_iter()
        .map(|(address, block_time, details)| {
            format!("Transaction: {}, {}, {:?}", address, block_time, details)
        })
        .collect();
    println!(
        "Contract instances stored:\n{}\n",
        instance_strings.join("\n")
    );
}

/// Queries the node available at `endpoint` from `height` for `Args.num_blocks` blocks. Inserts
/// results captured into a `DB` and prints the result.
async fn use_node(db: &mut DB) -> anyhow::Result<()> {
    let args = Args::parse();
    let endpoint = args.node;
    let current_height = AbsoluteBlockHeight { height: 0 }; // TOOD: get this from actual DB
    let blocks_to_process = current_height.height + args.num_blocks;

    println!("Using node {}\n", endpoint.uri());

    let mut node = Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(current_height)
        .await
        .context("Error querying blocks")?;

    if current_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            process_genesis_block(&mut node, genesis_block.block_hash, db).await?;
        }
    }

    for _ in current_height.height + 1..blocks_to_process {
        if let Some(block) = blocks_stream.next().await {
            process_block(&mut node, block.block_hash, db).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut db = DB {
        blocks: HashMap::new(),
        accounts: HashMap::new(),
        account_transactions: HashMap::new(),
        contract_modules: HashMap::new(),
        contract_instances: HashMap::new(),
    };

    use_node(&mut db)
        .await
        .context("Error happened while querying node.")?;

    print_db(db);

    Ok(())
}

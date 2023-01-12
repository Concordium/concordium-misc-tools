use std::collections::{BTreeMap, HashMap};
use std::hash::Hash;

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::types::hashes::TransactionMarker;
use concordium_rust_sdk::types::{
    AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary, ContractAddress,
    TransactionType,
};
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::AccountAddress,
    types::{
        hashes::{BlockMarker, HashBytes},
        AbsoluteBlockHeight,
        BlockItemSummaryDetails::{AccountCreation, AccountTransaction},
    },
    v2::{AccountIdentifier, Client, Endpoint, FinalizedBlockInfo},
};
use futures::{self, future, Stream, StreamExt, TryStreamExt};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(long, default_value = "http://localhost:20001")]
    node: Endpoint,
    /// How many blocks to process.
    // Only here for testing purposes...
    #[arg(long, default_value_t = 10000)]
    num_blocks: u64,
}

// Blocks are stored, so other tables can reference information about the block they were created in.
#[derive(Debug)]
struct BlockDetails {
    block_time: DateTime<Utc>,
    height: u64, // Used as a reference from where to restart on service restart.
}

#[derive(Debug)]
struct TransactionDetails {
    transaction_type: TransactionType,
    block_hash: String, // FK to blocks
    cost: u64,
}

#[derive(Debug)]
struct AccountDetails {
    is_initial: bool,
    block_hash: String, // FK to transactions
}

#[derive(Debug)]
struct ContractModuleDetails {
    block_hash: String, // FK to transactions
}

#[derive(Debug)]
struct ContractInstanceDetails {
    module_ref: String, // FK to modules
    block_hash: String, // FK to transactions
}

type BlocksTable = HashMap<String, BlockDetails>;
type AccountsTable = HashMap<CanonicalAccountAddress, AccountDetails>;
type AccountTransactionsTable = HashMap<String, TransactionDetails>;
type ContractModulesTable = HashMap<String, ContractModuleDetails>;
type ContractInstancesTable = HashMap<ContractAddress, ContractInstanceDetails>;

struct DB {
    blocks: BlocksTable,
    accounts: AccountsTable,
    account_transactions: AccountTransactionsTable,
    contract_modules: ContractModulesTable,
    contract_instances: ContractInstancesTable,
}

enum BlockEvent {
    AccountCreation(CanonicalAccountAddress, AccountDetails),
    AccountTransaction(String, TransactionDetails),
    ContractModuleDeployment(String, ContractModuleDetails),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
}

#[derive(Eq, Debug, Clone, Copy, Ord, PartialOrd)]
struct CanonicalAccountAddress(AccountAddress);

impl From<CanonicalAccountAddress> for AccountAddress {
    fn from(caa: CanonicalAccountAddress) -> Self {
        caa.0
    }
}

impl PartialEq for CanonicalAccountAddress {
    fn eq(&self, other: &Self) -> bool {
        let bytes_1: &[u8; 32] = self.0.as_ref();
        let bytes_2: &[u8; 32] = other.0.as_ref();
        bytes_1[0..29] == bytes_2[0..29]
    }
}

impl Hash for CanonicalAccountAddress {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        let bytes: &[u8; 32] = self.0.as_ref();
        bytes[0..29].hash(state);
    }
}

impl AsRef<CanonicalAccountAddress> for AccountAddress {
    fn as_ref(&self) -> &CanonicalAccountAddress {
        unsafe { std::mem::transmute(self) }
    }
}

async fn get_account_details(
    address: AccountAddress,
    block_hash: &HashBytes<BlockMarker>,
    node: &mut Client,
) -> anyhow::Result<AccountDetails> {
    let account_info = node
        .get_account_info(&AccountIdentifier::Address(address), block_hash)
        .await
        .context("Could not get account info")?
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

/// Returns a Map of AccountAddress, AccountDetails pairs included in block identified by
/// `block_hash`
async fn accounts_in_block(
    node: &mut Client,
    block_hash: &HashBytes<BlockMarker>,
) -> anyhow::Result<BTreeMap<CanonicalAccountAddress, AccountDetails>> {
    let mut accounts = node
        .get_account_list(block_hash)
        .await
        .context(format!("Block not found: {}", block_hash))?
        .response;

    let mut new_accounts = BTreeMap::new();

    while let Some(res) = accounts.next().await {
        let account = res.context("What exactly is this status error?")?; // TODO
        let key = CanonicalAccountAddress(account);
        let account_details = get_account_details(account, block_hash, node).await?;

        println!(
            "ACCOUNT:\naccount: {}\ndetails: {:?}",
            &key.0, &account_details,
        );

        new_accounts.insert(key, account_details);
    }

    Ok(new_accounts)
}

fn get_account_transaction_details(
    details: &AccountTransactionDetails,
    block_hash: &HashBytes<BlockMarker>,
) -> Option<TransactionDetails> {
    details
        .transaction_type()
        .map(|transaction_type| TransactionDetails {
            block_hash: block_hash.to_string(),
            transaction_type,
            cost: details.cost.micro_ccd,
        })
}

/// Maps `BlockItemSummary` to `BlockEvent`, which represent entities stored in the database.
async fn to_block_events(
    node: &mut Client,
    block_hash: &HashBytes<BlockMarker>,
    block_item: BlockItemSummary,
) -> Vec<anyhow::Result<BlockEvent>> {
    let mut events: Vec<anyhow::Result<BlockEvent>> = Vec::new();

    match block_item.details {
        AccountTransaction(atd) => {
            // TODO: do we want to store failed transactions?
            if let Some(details) = get_account_transaction_details(&atd, block_hash) {
                println!(
                    "TRANSACTION:\nhash: {}\ndetails: {:?}",
                    &block_item.hash, &details
                );
                let event = BlockEvent::AccountTransaction(block_item.hash.to_string(), details);
                events.push(Ok(event));
            }

            match atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let details = ContractModuleDetails {
                        block_hash: block_hash.to_string(),
                    };
                    println!(
                        "CONTRACT MODULE:\nref: {}\ndetails: {:?}",
                        &module_ref, &details
                    );
                    let event =
                        BlockEvent::ContractModuleDeployment(module_ref.to_string(), details);
                    events.push(Ok(event));
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        block_hash: block_hash.to_string(),
                        module_ref: data.origin_ref.to_string(),
                    };
                    println!(
                        "CONTRACT INSTANCE:\naddress: {}\ndetails: {:?}",
                        &data.address, &details
                    );
                    let event = BlockEvent::ContractInstantiation(data.address, details);
                    events.push(Ok(event));
                }
                _ => {}
            };
        }
        AccountCreation(act) => {
            let cad = CanonicalAccountAddress(act.address);
            let result = get_account_details(act.address, block_hash, node)
                .await
                .map(|details| {
                    // `.inspect` is marked as unstable
                    println!("ACCOUNT:\naddress: {}\ndetails: {:?}", &cad.0, &details);
                    details
                })
                .map(|details| BlockEvent::AccountCreation(cad, details));

            events.push(result);
        }
        _ => {}
    };

    events
}

// Don't know why I need explicit lifetime anotations here?
fn process_transactions<'a>(
    node: &'a mut Client,
    block_hash: &'a HashBytes<BlockMarker>,
    transactions_stream: impl Stream<Item = Result<BlockItemSummary, tonic::Status>> + 'a,
) -> impl Stream<Item = anyhow::Result<BlockEvent>> + 'a {
    let block_events_stream = transactions_stream
        .then(|res| {
            let mut node = node.clone();
            let c_block_hash = block_hash.clone();

            async move {
                let block_events = match res {
                    Ok(bi) => to_block_events(&mut node, &c_block_hash, bi).await,
                    Err(err) => vec![Err(anyhow!("Error while streaming block items: {}", err))],
                };

                futures::stream::iter(block_events)
            }
        })
        .flatten();

    block_events_stream
}

/// Insert as a single DB transaction to facilitate easy recovery, as the service can restart from
/// current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (&HashBytes<BlockMarker>, BlockDetails),
    accounts: BTreeMap<CanonicalAccountAddress, AccountDetails>,
    transactions: BTreeMap<String, TransactionDetails>,
    contract_modules: BTreeMap<String, ContractModuleDetails>,
    contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails>,
) {
    db.blocks.insert(block_hash.to_string(), block_details);
    db.accounts.extend(accounts.into_iter());
    db.account_transactions.extend(transactions.into_iter());
    db.contract_modules.extend(contract_modules.into_iter());
    db.contract_instances.extend(contract_instances.into_iter());
}

async fn handle_block(
    node: &mut Client,
    block: FinalizedBlockInfo,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block.block_hash)
        .await
        .context(format!("Block not found: {}", block.block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height.height,
    };

    let transactions_stream = node
        .get_block_transaction_events(block_info.block_hash)
        .await
        .context(format!("Block not found: {}", block_info.block_hash))?
        .response;

    let mut accounts: BTreeMap<CanonicalAccountAddress, AccountDetails> = BTreeMap::new();
    let mut account_transactions: BTreeMap<String, TransactionDetails> = BTreeMap::new();
    let mut contract_modules: BTreeMap<String, ContractModuleDetails> = BTreeMap::new();
    let mut contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails> =
        BTreeMap::new();

    if block.height.height == 0 {
        let mut genesis_accounts = accounts_in_block(node, &block.block_hash).await?;
        accounts.append(&mut genesis_accounts);
    }

    let block_events = process_transactions(node, &block_info.block_hash, transactions_stream);

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
        (&block_info.block_hash, block_details),
        accounts,
        account_transactions,
        contract_modules,
        contract_instances,
    );

    Ok(())
}

fn print_db(db: DB) {
    println!("Blocks stored: {}\n", &db.blocks.len());

    let mut accounts: Vec<(AccountAddress, DateTime<Utc>, AccountDetails)> = db
        .accounts
        .into_iter()
        .map(|(address_eq, details)| {
            let block_time = db
                .blocks
                .get(&details.block_hash)
                .expect("Found account with wrong reference to block?")
                .block_time;

            (address_eq.0, block_time, details)
        })
        .collect();

    accounts.sort_by_cached_key(|v| v.1);

    let account_strings: Vec<String> = accounts
        .into_iter()
        .map(|(address, block_time, details)| {
            format!("Account: {}, {}, {:?}", address, block_time, details)
        })
        .collect();

    println!("Accounts stored:\n{}", account_strings.join("\n"));

    let mut transactions: Vec<(String, DateTime<Utc>, TransactionDetails)> = db
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

    transactions.sort_by_cached_key(|v| v.1);

    let transaction_strings: Vec<String> = transactions
        .into_iter()
        .map(|(hash, block_time, details)| {
            format!("Transaction: {}, {}, {:?}", hash, block_time, details)
        })
        .collect();
    println!("Transactions stored:{}\n", transaction_strings.join("\n"));
}

async fn use_node(db: &mut DB) -> anyhow::Result<()> {
    let args = Args::parse();
    let endpoint = args.node;
    let current_height = 0; // TOOD: get this from actual DB

    println!("Using node {}\n", endpoint.uri());

    let mut node = Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(AbsoluteBlockHeight {
            height: current_height,
        })
        .await
        .context("Error querying blocks")?;

    for _ in 0..args.num_blocks {
        // TODO: Make concurrent
        if let Some(block) = blocks_stream.next().await {
            handle_block(&mut node, block, db).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    let mut db = DB {
        blocks: HashMap::new(),
        accounts: HashMap::new(),
        account_transactions: HashMap::new(),
        contract_modules: HashMap::new(),
        contract_instances: HashMap::new(),
    };

    if let Err(error) = use_node(&mut db).await {
        println!("Error happened: {}", error)
    }

    print_db(db);
}

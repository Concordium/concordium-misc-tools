use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::{AccountAddress, Amount},
    types::{
        hashes::{BlockHash, TransactionHash},
        smart_contracts::ModuleRef,
        AbsoluteBlockHeight, AccountCreationDetails, AccountTransactionDetails,
        AccountTransactionEffects, BlockItemSummary,
        BlockItemSummaryDetails::{AccountCreation, AccountTransaction},
        ContractAddress, CredentialType, TransactionType,
    },
    v2::{AccountIdentifier, Client, Endpoint},
};
use futures::{self, future, Stream, StreamExt, TryStreamExt};
use tokio_postgres::{config::Config as DBConfig, Client as DBClient, NoTls};

#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoint is expected to point to a concordium node grpc v2 API.",
        default_value = "http://localhost:20001"
    )]
    node:          Endpoint,
    /// How many blocks to process.
    // Only here for testing purposes...
    #[arg(long = "num-blocks", default_value_t = 10000)]
    num_blocks:    u64,
    /// Database connection string.
    #[arg(
        long = "db-connection",
        default_value = "host=localhost dbname=kpi-tracker user=postgres password=password \
                         port=5432"
    )]
    db_connection: DBConfig,
}

/// Information about individual blocks. Useful for linking entities to a block
/// and it's corresponding attributes.
#[derive(Debug, Clone, Copy)]
struct BlockDetails {
    /// Finalization time of the block. Used to show how metrics evolve over
    /// time by linking entities, such as accounts and transactions, to
    /// the block in which they are created.
    block_time: DateTime<Utc>,
    /// Height of block from genesis. Used to restart the process of collecting
    /// metrics from the latest block recorded.
    height:     AbsoluteBlockHeight,
}

/// Holds selected attributes about accounts created on chain.
#[derive(Debug)]
struct AccountDetails {
    /// Whether an account was created as an initial account or not.
    is_initial: bool,
    /// Foreign key to the block in which the account was created.
    block_hash: BlockHash,
}

/// Holds selected attributes of an account transaction.
#[derive(Debug)]
struct TransactionDetails {
    /// The transaction type of the account transaction. Can be none if
    /// transaction was rejected due to serialization failure.
    transaction_type: Option<TransactionType>,
    /// Foreign key to the block in which the transaction was finalized.
    block_hash:       BlockHash,
    /// The cost of the transaction.
    cost:             Amount,
    /// Whether the transaction failed or not.
    is_success:       bool,
}

/// Holds selected attributes of a contract module deployed on chain.
#[derive(Debug)]
struct ContractModuleDetails {
    /// Foreign key to the block in which the module was deployed.
    block_hash: BlockHash,
}

/// Holds selected attributes of a contract instance created on chain.
#[derive(Debug)]
struct ContractInstanceDetails {
    /// Foreign key to the module used to instantiate the contract
    module_ref: ModuleRef,
    /// Foreign key to the block in which the contract was instantiated.
    block_hash: BlockHash,
}

type BlocksTable = HashMap<BlockHash, BlockDetails>;
type AccountsTable = HashMap<AccountAddress, AccountDetails>;
type AccountTransactionsTable = HashMap<TransactionHash, TransactionDetails>;
type ContractModulesTable = HashMap<ModuleRef, ContractModuleDetails>;
type ContractInstancesTable = HashMap<ContractAddress, ContractInstanceDetails>;

/// This is intended as a in-memory DB, which follows the same schema as the
/// final DB will follow.
struct DB {
    /// Table containing all blocks queried from node.
    blocks:               BlocksTable,
    /// Table containing all accounts created on chain, along with accounts
    /// present in genesis block
    accounts:             AccountsTable,
    /// Table containing all account transactions finalized on chain.
    account_transactions: AccountTransactionsTable,
    /// Table containing all smart contract modules deployed on chain.
    contract_modules:     ContractModulesTable,
    /// Table containing all smart contract instances created on chain.
    contract_instances:   ContractInstancesTable,
}

struct DBConn {
    client: DBClient,
}

impl DBConn {
    async fn create(conn_string: DBConfig, try_create_tables: bool) -> anyhow::Result<Self> {
        let (client, connection) = conn_string
            .connect(NoTls)
            .await
            .context("Could not create database connection")?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e); // TODO: log as error.
            }
        });

        if try_create_tables {
            let create_statements = include_str!("../resources/schema.sql");
            client
                .batch_execute(create_statements)
                .await
                .context("Failed to execute create statements")?;
        }

        let db_conn = DBConn { client };
        Ok(db_conn)
    }
}

/// Events from individual transactions to store in the database.
enum BlockEvent {
    AccountCreation(AccountAddress, AccountDetails),
    AccountTransaction(TransactionHash, TransactionDetails),
    ContractModuleDeployment(ModuleRef, ContractModuleDetails),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
}

/// Queries node for account info for the `account` given at the block
/// represented by the `block_hash`
fn account_details(
    block_hash: BlockHash,
    account_creation_details: AccountCreationDetails,
) -> AccountDetails {
    let is_initial = match account_creation_details.credential_type {
        CredentialType::Initial { .. } => true,
        CredentialType::Normal { .. } => false,
    };

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
                    format!("Error while streaming accounts in block: {}", block_hash)
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

/// Maps `AccountTransactionDetails` to `TransactionDetails`, where rejected
/// transactions without a transaction type are represented by `None`.
fn get_account_transaction_details(
    details: &AccountTransactionDetails,
    block_hash: BlockHash,
) -> TransactionDetails {
    let transaction_type = details.transaction_type();
    let is_success = details.effects.is_rejected().is_none();

    TransactionDetails {
        block_hash,
        transaction_type,
        is_success,
        cost: details.cost,
    }
}

/// Maps `BlockItemSummary` to `Vec<BlockEvent>`, which represent entities
/// stored in the database.
fn to_block_events(block_hash: BlockHash, block_item: BlockItemSummary) -> Vec<BlockEvent> {
    let mut events: Vec<BlockEvent> = Vec::new();

    match block_item.details {
        AccountTransaction(atd) => {
            let details = get_account_transaction_details(&atd, block_hash);
            println!(
                "TRANSACTION:\nhash: {}\ndetails: {:?}",
                block_item.hash, &details
            ); // Logger debug
            let event = BlockEvent::AccountTransaction(block_item.hash, details);
            events.push(event);

            match atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let details = ContractModuleDetails { block_hash };
                    println!(
                        "CONTRACT MODULE:\nref: {}\ndetails: {:?}",
                        &module_ref, &details
                    ); // Logger debug
                    let event = BlockEvent::ContractModuleDeployment(module_ref, details);
                    events.push(event);
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        block_hash,
                        module_ref: data.origin_ref,
                    };
                    println!(
                        "CONTRACT INSTANCE:\naddress: {}\ndetails: {:?}",
                        &data.address, &details
                    ); // Logger debug
                    let event = BlockEvent::ContractInstantiation(data.address, details);
                    events.push(event);
                }
                _ => {}
            };
        }
        AccountCreation(act) => {
            let address = act.address;
            let details = account_details(block_hash, act);

            println!("ACCOUNT:\naddress: {}\ndetails: {:?}", &address, &details); // Logger debug

            let block_event = BlockEvent::AccountCreation(address, details);
            events.push(block_event);
        }
        _ => {}
    };

    events
}

/// Maps a stream of transactions to a stream of `BlockEvent`s
async fn get_block_events(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<impl Stream<Item = anyhow::Result<BlockEvent>>> {
    let transactions = node
        .get_block_transaction_events(block_hash)
        .await
        .with_context(|| format!("Could not get transactions for block: {}", block_hash))?
        .response;

    let block_events = transactions
        .map_ok(move |bi| {
            let block_events: Vec<Result<BlockEvent, anyhow::Error>> =
                to_block_events(block_hash, bi)
                    .into_iter()
                    .map(Ok)
                    .collect();
            futures::stream::iter(block_events)
        })
        .map_err(move |err| {
            anyhow!(
                "Error while streaming transactions for block: {} - {}",
                block_hash,
                err
            )
        })
        .try_flatten();

    Ok(block_events)
}

/// Init db with a block and corresponding accounts found in block. This is a
/// helper function to be used for genesis block only.
fn init_db(
    db: &mut DB,
    (block_hash, block_details): (BlockHash, BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
) {
    db.blocks.insert(block_hash, block_details);
    db.accounts.extend(accounts.into_iter());
}

/// Insert as a single DB transaction to facilitate easy recovery, as the
/// service can restart from current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (BlockHash, BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
    transactions: BTreeMap<TransactionHash, TransactionDetails>,
    contract_modules: BTreeMap<ModuleRef, ContractModuleDetails>,
    contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails>,
) {
    db.blocks.insert(block_hash, block_details);
    db.accounts.extend(accounts.into_iter());
    db.account_transactions.extend(transactions.into_iter());
    db.contract_modules.extend(contract_modules.into_iter());
    db.contract_instances.extend(contract_instances.into_iter());
}

/// Processes a block, represented by `block_hash` by querying the node for
/// entities present in the block state, updating the `db`. Should only be
/// used to process the genesis block.
async fn process_genesis_block(
    node: &mut Client,
    block_hash: BlockHash,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for genesis block: {}", block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height:     block_info.block_height,
    };

    let genesis_accounts = accounts_in_block(node, block_hash).await?;
    init_db(db, (block_hash, block_details), genesis_accounts);

    Ok(())
}

/// Process a block, represented by `block_hash`, updating the `db`
/// corresponding to events captured by the block.
async fn process_block(
    node: &mut Client,
    block_hash: BlockHash,
    db: &mut DB,
) -> anyhow::Result<()> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for block: {}", block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height:     block_info.block_height,
    };

    let mut accounts: BTreeMap<AccountAddress, AccountDetails> = BTreeMap::new();
    let mut account_transactions: BTreeMap<TransactionHash, TransactionDetails> = BTreeMap::new();
    let mut contract_modules: BTreeMap<ModuleRef, ContractModuleDetails> = BTreeMap::new();
    let mut contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails> =
        BTreeMap::new();

    let block_events = get_block_events(node, block_info.block_hash).await?;
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
        (block_hash, block_details),
        accounts,
        account_transactions,
        contract_modules,
        contract_instances,
    );

    Ok(())
}

/// Prints the state of the `db` given.
fn print_db(db: DB) {
    // Print blocks
    println!("{} blocks stored\n", &db.blocks.len());

    let get_block_time = |block_hash: BlockHash| {
        db.blocks
            .get(&block_hash)
            .expect("Entity with wrong reference to block")
            .block_time
    };

    // Print accounts
    let mut accounts: Vec<(AccountAddress, DateTime<Utc>, AccountDetails)> = db
        .accounts
        .into_iter()
        .map(|(address, details)| (address, get_block_time(details.block_hash), details))
        .collect();

    accounts.sort_by_key(|v| v.1);

    let account_strings: Vec<String> = accounts
        .into_iter()
        .map(|(address, block_time, details)| {
            format!("Account: {}, {}, {:?}", address, block_time, details)
        })
        .collect();
    println!(
        "{} accounts stored:\n{}\n",
        account_strings.len(),
        account_strings.join("\n")
    );

    // Print transactions
    let mut transactions: Vec<(TransactionHash, DateTime<Utc>, TransactionDetails)> = db
        .account_transactions
        .into_iter()
        .map(|(hash, details)| (hash, get_block_time(details.block_hash), details))
        .collect();

    transactions.sort_by_key(|v| v.1);

    let transaction_strings: Vec<String> = transactions
        .into_iter()
        .map(|(hash, block_time, details)| {
            format!("Transaction: {}, {}, {:?}", hash, block_time, details)
        })
        .collect();
    println!(
        "{} transactions stored:\n{}\n",
        transaction_strings.len(),
        transaction_strings.join("\n")
    );

    // Print contract modules
    let mut contract_modules: Vec<(ModuleRef, DateTime<Utc>, ContractModuleDetails)> = db
        .contract_modules
        .into_iter()
        .map(|(m_ref, details)| (m_ref, get_block_time(details.block_hash), details))
        .collect();

    contract_modules.sort_by_key(|v| v.1);

    let module_strings: Vec<String> = contract_modules
        .into_iter()
        .map(|(m_ref, block_time, details)| {
            format!("Contract module: {}, {}, {:?}", m_ref, block_time, details)
        })
        .collect();
    println!(
        "{} contract modules stored:\n{}\n",
        module_strings.len(),
        module_strings.join("\n")
    );

    // Print contract instances
    let mut contract_instances: Vec<(ContractAddress, DateTime<Utc>, ContractInstanceDetails)> = db
        .contract_instances
        .into_iter()
        .map(|(address, details)| (address, get_block_time(details.block_hash), details))
        .collect();

    contract_instances.sort_by_key(|v| v.1);

    let instance_strings: Vec<String> = contract_instances
        .into_iter()
        .map(|(address, block_time, details)| {
            format!(
                "Contract instance: {}, {}, {:?}",
                address, block_time, details
            )
        })
        .collect();
    println!(
        "{} contract instances stored:\n{}\n",
        instance_strings.len(),
        instance_strings.join("\n")
    );
}

/// Queries the node available at `Args.endpoint` from `from_height` for
/// `Args.num_blocks` blocks. Inserts results captured into the supplied `db`.
async fn use_node(db: &mut DB, from_height: AbsoluteBlockHeight) -> anyhow::Result<()> {
    let args = Args::parse();
    let endpoint = args.node;
    let blocks_to_process = from_height.height + args.num_blocks;

    println!("Using node {}\n", endpoint.uri());

    let mut node = Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(from_height)
        .await
        .context("Error querying blocks")?;

    if from_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            process_genesis_block(&mut node, genesis_block.block_hash, db).await?;
        }
    }

    for _ in from_height.height + 1..blocks_to_process {
        if let Some(block) = blocks_stream.next().await {
            process_block(&mut node, block.block_hash, db).await?;
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let _ = DBConn::create(args.db_connection, true).await?;

    let mut db = DB {
        blocks:               HashMap::new(),
        accounts:             HashMap::new(),
        account_transactions: HashMap::new(),
        contract_modules:     HashMap::new(),
        contract_instances:   HashMap::new(),
    };

    let current_height = AbsoluteBlockHeight { height: 0 }; // TOOD: get this from actual DB
    use_node(&mut db, current_height)
        .await
        .context("Error happened while querying node.")?;

    print_db(db);

    Ok(())
}

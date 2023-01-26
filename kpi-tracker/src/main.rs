use core::fmt;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::{AccountAddress, Amount, ACCOUNT_ADDRESS_SIZE},
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
    /// Logging level of the application
    #[arg(long = "log-level", default_value = "debug")]
    log_level:     log::LevelFilter,
}

#[derive(Eq, PartialEq, Copy, Clone, PartialOrd, Ord, Debug, Hash)]
struct CanonicalAccountAddress([u8; ACCOUNT_ADDRESS_SIZE]);

impl From<AccountAddress> for CanonicalAccountAddress {
    fn from(aa: AccountAddress) -> Self {
        let bytes: &[u8; ACCOUNT_ADDRESS_SIZE] = aa.as_ref();
        let mut canonical_bytes = [0u8; ACCOUNT_ADDRESS_SIZE];

        canonical_bytes[..29].copy_from_slice(&bytes[..29]);
        CanonicalAccountAddress(canonical_bytes)
    }
}
impl fmt::Display for CanonicalAccountAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { AccountAddress(self.0).fmt(f) }
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

/// Represents a relation between an account and a transaction
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
struct TransactionAccountRelation {
    account:     CanonicalAccountAddress,
    transaction: TransactionHash,
}

/// Represents a relation between a contract and a transaction
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
struct TransactionContractRelation {
    contract:    ContractAddress,
    transaction: TransactionHash,
}

type Blocks = HashMap<BlockHash, BlockDetails>;
type Accounts = HashMap<CanonicalAccountAddress, AccountDetails>;
type AccountTransactions = HashMap<TransactionHash, TransactionDetails>;
type ContractModules = HashMap<ModuleRef, ContractModuleDetails>;
type ContractInstances = HashMap<ContractAddress, ContractInstanceDetails>;
type TransactionAccountRelations = HashSet<TransactionAccountRelation>;
type TransactionContractRelations = HashSet<TransactionContractRelation>;

#[derive(Debug)]
struct GenesisBlockData {
    block_hash:    BlockHash,
    block_details: BlockDetails,
    accounts:      Accounts,
}

#[derive(Debug)]
struct NormalBlockData {
    block_hash: BlockHash,
    block_details: BlockDetails,
    accounts: Accounts,
    transactions: AccountTransactions,
    contract_modules: ContractModules,
    contract_instances: ContractInstances,
    transaction_account_relations: TransactionAccountRelations,
    transaction_contract_relations: TransactionContractRelations,
}

#[derive(Debug)]
enum BlockData {
    Genesis(GenesisBlockData),
    Normal(NormalBlockData),
}

/// This is intended as a in-memory DB, which follows the same schema as the
/// final DB will follow.
struct DB {
    /// Table containing all blocks queried from node.
    blocks: Blocks,
    /// Table containing all accounts created on chain, along with accounts
    /// present in genesis block
    accounts: Accounts,
    /// Table containing all account transactions finalized on chain.
    account_transactions: AccountTransactions,
    /// Table containing all smart contract modules deployed on chain.
    contract_modules: ContractModules,
    /// Table containing all smart contract instances created on chain.
    contract_instances: ContractInstances,
    /// Table containing relations between accounts and transactions.
    transaction_account_relations: TransactionAccountRelations,
    /// Table containing relations between contract instances and transactions.
    transaction_contract_relations: TransactionContractRelations,
}

struct PreparedStatements {
    insert_block: tokio_postgres::Statement,
    insert_account: tokio_postgres::Statement,
    insert_contract_module: tokio_postgres::Statement,
    insert_contract_instance: tokio_postgres::Statement,
    insert_transaction: tokio_postgres::Statement,
    insert_account_transaction_relation: tokio_postgres::Statement,
    insert_contract_transaction_relation: tokio_postgres::Statement,
}

struct DBConn {
    client:   DBClient,
    prepared: PreparedStatements,
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

        let insert_block = client
            .prepare(
                "INSERT INTO blocks (hash, timestamp, height, total_stake) VALUES ($1, $2, $3, \
                 $4) RETURNING id",
            )
            .await?;
        let insert_account = client
            .prepare(
                "INSERT INTO accounts (address, block, is_initial) VALUES ($1, $2, $3) RETURNING \
                 id",
            )
            .await?;
        let insert_contract_module = client
            .prepare("INSERT INTO modules (ref, block) VALUES ($1, $2) RETURNING id")
            .await?;
        let insert_contract_instance = client
            .prepare(
                "INSERT INTO contracts (index, subindex, module, block) VALUES ($1, $2, $3, $4) \
                 RETURNING id",
            )
            .await?;
        let insert_transaction = client
            .prepare("INSERT INTO blocks (hash, block, type) VALUES ($1, $2, $3) RETURNING id")
            .await?;
        let insert_account_transaction_relation = client
            .prepare("INSERT INTO accounts_transactions (account, transaction) VALUES ($1, $2)")
            .await?;
        let insert_contract_transaction_relation = client
            .prepare("INSERT INTO contracts_transactions (contract, transaction) VALUES ($1, $2)")
            .await?;

        let prepared = PreparedStatements {
            insert_block,
            insert_account,
            insert_contract_module,
            insert_contract_instance,
            insert_transaction,
            insert_account_transaction_relation,
            insert_contract_transaction_relation,
        };

        let db_conn = DBConn { client, prepared };
        Ok(db_conn)
    }
}

/// Events from individual transactions to store in the database.
enum BlockEvent {
    AccountCreation(AccountAddress, AccountDetails),
    AccountTransaction(
        TransactionHash,
        TransactionDetails,
        Vec<TransactionAccountRelation>,
        Vec<TransactionContractRelation>,
    ),
    ContractModuleDeployment(ModuleRef, ContractModuleDetails),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
}

/// Queries node for account info for the `account` given at the block
/// represented by the `block_hash`
fn account_details(
    block_hash: BlockHash,
    account_creation_details: &AccountCreationDetails,
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
) -> anyhow::Result<HashMap<CanonicalAccountAddress, AccountDetails>> {
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
        .try_fold(HashMap::new(), |mut map, (account, info)| async move {
            let is_initial = info
                .account_credentials
                .get(&0.into())
                .map_or(false, |cdi| match cdi.value {
                    AccountCredentialWithoutProofs::Initial { .. } => true,
                    AccountCredentialWithoutProofs::Normal { .. } => false,
                });

            let canonical_account = CanonicalAccountAddress::from(account);
            let details = AccountDetails {
                is_initial,
                block_hash,
            };
            map.insert(canonical_account, details);

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

    match &block_item.details {
        AccountTransaction(atd) => {
            let details = get_account_transaction_details(atd, block_hash);
            let affected_accounts: Vec<TransactionAccountRelation> = block_item
                .affected_addresses()
                .into_iter()
                .map(|address| TransactionAccountRelation {
                    account:     CanonicalAccountAddress::from(address),
                    transaction: block_item.hash,
                })
                .collect();

            let affected_contracts: Vec<TransactionContractRelation> = block_item
                .affected_contracts()
                .into_iter()
                .map(|contract| TransactionContractRelation {
                    contract,
                    transaction: block_item.hash,
                })
                .collect();

            let event = BlockEvent::AccountTransaction(
                block_item.hash,
                details,
                affected_accounts,
                affected_contracts,
            );

            events.push(event);

            match &atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let details = ContractModuleDetails { block_hash };
                    let event = BlockEvent::ContractModuleDeployment(*module_ref, details);

                    events.push(event);
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        block_hash,
                        module_ref: data.origin_ref,
                    };
                    let event = BlockEvent::ContractInstantiation(data.address, details);

                    events.push(event);
                }
                _ => {}
            };
        }
        AccountCreation(acd) => {
            let details = account_details(block_hash, acd);
            let block_event = BlockEvent::AccountCreation(acd.address, details);

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
    accounts: HashMap<CanonicalAccountAddress, AccountDetails>,
) {
    db.blocks.insert(block_hash, block_details);
    db.accounts.extend(accounts.into_iter());
}

/// Insert as a single DB transaction to facilitate easy recovery, as the
/// service can restart from current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (BlockHash, BlockDetails),
    accounts: HashMap<CanonicalAccountAddress, AccountDetails>,
    transactions: HashMap<TransactionHash, TransactionDetails>,
    contract_modules: HashMap<ModuleRef, ContractModuleDetails>,
    contract_instances: HashMap<ContractAddress, ContractInstanceDetails>,
    transaction_account_relations: HashSet<TransactionAccountRelation>,
    transaction_contract_relations: HashSet<TransactionContractRelation>,
) {
    db.blocks.insert(block_hash, block_details);
    db.accounts.extend(accounts.into_iter());
    db.account_transactions.extend(transactions.into_iter());
    db.contract_modules.extend(contract_modules.into_iter());
    db.contract_instances.extend(contract_instances.into_iter());
    db.transaction_account_relations
        .extend(transaction_account_relations.into_iter());
    db.transaction_contract_relations
        .extend(transaction_contract_relations.into_iter());
}

/// Processes a block, represented by `block_hash` by querying the node for
/// entities present in the block state, updating the `db`. Should only be
/// used to process the genesis block.
async fn process_genesis_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<GenesisBlockData> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for genesis block: {}", block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height:     block_info.block_height,
    };

    let accounts = accounts_in_block(node, block_hash).await?;
    let genesis_data = GenesisBlockData {
        block_hash,
        block_details,
        accounts,
    };

    Ok(genesis_data)
}

/// Process a block, represented by `block_hash`, updating the `db`
/// corresponding to events captured by the block.
async fn process_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<NormalBlockData> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for block: {}", block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height:     block_info.block_height,
    };

    let mut block_data = NormalBlockData {
        block_hash,
        block_details,
        accounts: HashMap::new(),
        transactions: HashMap::new(),
        contract_modules: HashMap::new(),
        contract_instances: HashMap::new(),
        transaction_account_relations: HashSet::new(),
        transaction_contract_relations: HashSet::new(),
    };

    let block_events = get_block_events(node, block_info.block_hash).await?;
    block_events
        .try_for_each(|be| {
            match be {
                BlockEvent::AccountCreation(address, details) => {
                    block_data
                        .accounts
                        .insert(CanonicalAccountAddress::from(address), details);
                }
                BlockEvent::AccountTransaction(
                    hash,
                    details,
                    affected_accounts,
                    affected_contracts,
                ) => {
                    block_data.transactions.insert(hash, details);
                    block_data
                        .transaction_account_relations
                        .extend(affected_accounts.into_iter());
                    block_data
                        .transaction_contract_relations
                        .extend(affected_contracts.into_iter());
                }
                BlockEvent::ContractModuleDeployment(module_ref, details) => {
                    block_data.contract_modules.insert(module_ref, details);
                }
                BlockEvent::ContractInstantiation(address, details) => {
                    block_data.contract_instances.insert(address, details);
                }
            };

            future::ok(())
        })
        .await?;

    Ok(block_data)
}

/// Queries the node available at `Args.endpoint` from `from_height` for
/// `Args.num_blocks` blocks. Inserts results captured into the supplied `db`.
async fn use_node(
    from_height: AbsoluteBlockHeight,
    sender: tokio::sync::mpsc::Sender<BlockData>,
) -> anyhow::Result<()> {
    let args = Args::parse();
    let endpoint = args.node;
    let blocks_to_process = from_height.height + args.num_blocks;

    log::info!(
        "Processing {} blocks from height {} using node {}",
        args.num_blocks,
        from_height,
        endpoint.uri()
    );

    let mut node = Client::new(endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(from_height)
        .await
        .context("Error querying blocks")?;

    if from_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            let genesis_block_data =
                process_genesis_block(&mut node, genesis_block.block_hash).await?;
            sender.send(BlockData::Genesis(genesis_block_data)).await?;
            log::info!("Processed genesis block: {}", genesis_block.block_hash);
        }
    }

    for height in from_height.height + 1..blocks_to_process {
        if let Some(block) = blocks_stream.next().await {
            let block_data = process_block(&mut node, block.block_hash).await?;
            sender.send(BlockData::Normal(block_data)).await?;
            log::info!("Processed block ({}): {}", height, block.block_hash);
        }
    }

    Ok(())
}

async fn db_writer(mut receiver: tokio::sync::mpsc::Receiver<BlockData>) {
    while let Some(block_data) = receiver.recv().await {
        println!("Block data received: {:?}", block_data);
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let _ = DBConn::create(args.db_connection, true).await?;
    env_logger::Builder::new()
        .filter_module(module_path!(), args.log_level) // Only log the current module (main).
        .init();

    // Create a channel between the task querying the node and the task logging
    // transactions.
    let (sender, receiver) = tokio::sync::mpsc::channel(100);

    tokio::spawn(db_writer(receiver));

    let current_height = AbsoluteBlockHeight { height: 0 }; // TOOD: get this from actual DB
    use_node(current_height, sender)
        .await
        .context("Error happened while querying node.")?;

    Ok(())
}

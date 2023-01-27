use core::fmt;
use std::collections::{HashMap, HashSet};

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
        ContractAddress, CredentialType, RewardsOverview, TransactionType,
    },
    v2::{AccountIdentifier, Client, Endpoint},
};
use futures::{self, future, Stream, StreamExt, TryStreamExt};
use tokio_postgres::{
    config::Config as DBConfig, types::ToSql, Client as DBClient, NoTls,
    Transaction as DBTransaction,
};

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
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Debug)]
    log_level:     log::LevelFilter,
    /// Block height to start collecting from
    #[arg(long = "from-height", default_value_t = 0)]
    from_height:   u64,
}

/// Used to canonicalize account addresses to ensure no aliases are stored (as
/// aliases are included in the affected accounts of transactions.)
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
    block_time:  DateTime<Utc>,
    /// Height of block from genesis. Used to restart the process of collecting
    /// metrics from the latest block recorded.
    height:      AbsoluteBlockHeight,
    /// Total amount staked across all pools inclusive passive delegation. This
    /// is only recorded for "payday" blocks reflected by `Some`, where non
    /// payday blocks are reflected by `None`.
    total_stake: Option<Amount>,
}

/// Holds selected attributes about accounts created on chain.
#[derive(Debug)]
struct AccountDetails {
    /// Whether an account was created as an initial account or not.
    is_initial: bool,
}

/// Holds selected attributes of an account transaction.
#[derive(Debug)]
struct TransactionDetails {
    /// The transaction type of the account transaction. Can be none if
    /// transaction was rejected due to serialization failure.
    transaction_type:   Option<TransactionType>,
    /// The cost of the transaction.
    cost:               Amount,
    /// Whether the transaction failed or not.
    is_success:         bool,
    /// Accounts affected by the transactions.
    affected_accounts:  Vec<CanonicalAccountAddress>,
    /// Contracts affected by the transactions.
    affected_contracts: Vec<ContractAddress>,
}

/// Holds selected attributes of a contract instance created on chain.
#[derive(Debug)]
struct ContractInstanceDetails {
    /// Foreign key to the module used to instantiate the contract
    module_ref: ModuleRef,
}

type Accounts = HashMap<CanonicalAccountAddress, AccountDetails>;
type AccountTransactions = HashMap<TransactionHash, TransactionDetails>;
type ContractModules = HashSet<ModuleRef>;
type ContractInstances = HashMap<ContractAddress, ContractInstanceDetails>;

#[derive(Debug)]
struct GenesisBlockData {
    block_hash:    BlockHash,
    block_details: BlockDetails,
    accounts:      Accounts,
}

#[derive(Debug)]
struct NormalBlockData {
    block_hash:         BlockHash,
    block_details:      BlockDetails,
    accounts:           Accounts,
    transactions:       AccountTransactions,
    contract_modules:   ContractModules,
    contract_instances: ContractInstances,
}

#[derive(Debug)]
enum BlockData {
    Genesis(GenesisBlockData),
    Normal(NormalBlockData),
}

struct PreparedStatements {
    insert_block: tokio_postgres::Statement,
    insert_account: tokio_postgres::Statement,
    insert_contract_module: tokio_postgres::Statement,
    insert_contract_instance: tokio_postgres::Statement,
    insert_transaction: tokio_postgres::Statement,
    insert_account_transaction_relation: tokio_postgres::Statement,
    insert_contract_transaction_relation: tokio_postgres::Statement,
    get_latest_height: tokio_postgres::Statement,
}

impl PreparedStatements {
    async fn insert_block<'a, 'b>(
        &'a self,
        tx: &DBTransaction<'b>,
        block_hash: BlockHash,
        block_details: BlockDetails,
    ) -> Result<i64, tokio_postgres::Error> {
        let total_stake = block_details
            .total_stake
            .map(|amount| (amount.micro_ccd() as i64));
        let values: [&(dyn ToSql + Sync); 4] = [
            &block_hash.as_ref(),
            &block_details.block_time.timestamp(),
            &(block_details.height.height as i64),
            &total_stake,
        ];

        let row = tx.query_one(&self.insert_block, &values).await?;
        let id = row.try_get::<_, i64>(0)?;

        Ok(id)
    }

    async fn get_latest_height(
        &self,
        db: &DBClient,
    ) -> Result<Option<AbsoluteBlockHeight>, tokio_postgres::Error> {
        let row = db.query_opt(&self.get_latest_height, &[]).await?;
        if let Some(row) = row {
            let raw = row.try_get::<_, i64>(0)?;
            Ok(Some((raw as u64).into()))
        } else {
            Ok(None)
        }
    }
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
            .prepare(
                "INSERT INTO transactions (hash, block, cost, type) VALUES ($1, $2, $3, $4) \
                 RETURNING id",
            )
            .await?;
        let insert_account_transaction_relation = client
            .prepare("INSERT INTO accounts_transactions (account, transaction) VALUES ($1, $2)")
            .await?;
        let insert_contract_transaction_relation = client
            .prepare("INSERT INTO contracts_transactions (contract, transaction) VALUES ($1, $2)")
            .await?;
        let get_latest_height = client
            .prepare("SELECT blocks.height FROM blocks ORDER BY blocks.id DESC LIMIT 1")
            .await?;

        let prepared = PreparedStatements {
            insert_block,
            insert_account,
            insert_contract_module,
            insert_contract_instance,
            insert_transaction,
            insert_account_transaction_relation,
            insert_contract_transaction_relation,
            get_latest_height,
        };

        let db_conn = DBConn { client, prepared };
        Ok(db_conn)
    }
}

/// Events from individual transactions to store in the database.
enum BlockEvent {
    AccountCreation(CanonicalAccountAddress, AccountDetails),
    AccountTransaction(TransactionHash, TransactionDetails),
    ContractModuleDeployment(ModuleRef),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
}

/// Queries node for account info for the `account` given at the block
/// represented by the `block_hash`
fn account_details(account_creation_details: &AccountCreationDetails) -> AccountDetails {
    let is_initial = match account_creation_details.credential_type {
        CredentialType::Initial { .. } => true,
        CredentialType::Normal { .. } => false,
    };

    AccountDetails { is_initial }
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
            let details = AccountDetails { is_initial };
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
    block_item: &BlockItemSummary,
) -> TransactionDetails {
    let transaction_type = details.transaction_type();
    let is_success = details.effects.is_rejected().is_none();
    let affected_accounts: Vec<CanonicalAccountAddress> = block_item
        .affected_addresses()
        .into_iter()
        .map(CanonicalAccountAddress::from)
        .collect();
    let affected_contracts = block_item.affected_contracts();

    TransactionDetails {
        transaction_type,
        is_success,
        cost: details.cost,
        affected_accounts,
        affected_contracts,
    }
}

/// Maps `BlockItemSummary` to `Vec<BlockEvent>`, which represent entities
/// stored in the database.
fn to_block_events(block_item: BlockItemSummary) -> Vec<BlockEvent> {
    let mut events: Vec<BlockEvent> = Vec::new();

    match &block_item.details {
        AccountTransaction(atd) => {
            let details = get_account_transaction_details(atd, &block_item);
            let event = BlockEvent::AccountTransaction(block_item.hash, details);
            events.push(event);

            match &atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let event = BlockEvent::ContractModuleDeployment(*module_ref);
                    events.push(event);
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        module_ref: data.origin_ref,
                    };
                    let event = BlockEvent::ContractInstantiation(data.address, details);
                    events.push(event);
                }
                _ => {}
            };
        }
        AccountCreation(acd) => {
            let details = account_details(acd);
            let block_event =
                BlockEvent::AccountCreation(CanonicalAccountAddress::from(acd.address), details);
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
                to_block_events(bi).into_iter().map(Ok).collect();
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
        block_time:  block_info.block_slot_time,
        height:      block_info.block_height,
        total_stake: None,
    };

    let accounts = accounts_in_block(node, block_hash).await?;
    let genesis_data = GenesisBlockData {
        block_hash,
        block_details,
        accounts,
    };

    Ok(genesis_data)
}

/// If block specified by `block_hash` is a payday block (also implies >=
/// protocol version 4), this returns the total stake for that block. Otherwise
/// returns `None`.
async fn p4_payday_total_stake(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<Option<Amount>> {
    let tokenomics_info = node
        .get_tokenomics_info(block_hash)
        .await
        .with_context(|| format!("Could not get tokenomics info for block: {}", block_hash))?
        .response;

    if let RewardsOverview::V1 {
        total_staked_capital,
        ..
    } = tokenomics_info
    {
        let is_payday_block = node
            .is_payday_block(block_hash)
            .await
            .with_context(|| {
                format!(
                    "Could not assert whether block is payday block for: {}",
                    block_hash
                )
            })?
            .response;

        if is_payday_block {
            return Ok(Some(total_staked_capital));
        };

        return Ok(None);
    }

    Ok(None)
}

/// Get `BlockDetails` for given block represented by `block_hash`
async fn get_block_details(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<BlockDetails> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for block: {}", block_hash))?
        .response;

    let total_stake = p4_payday_total_stake(node, block_hash).await?;
    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
        total_stake,
    };

    Ok(block_details)
}

/// Process a block, represented by `block_hash`, updating the `db`
/// corresponding to events captured by the block.
async fn process_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<NormalBlockData> {
    let block_details = get_block_details(node, block_hash).await?;
    let block_events = get_block_events(node, block_hash).await?;

    let mut block_data = NormalBlockData {
        block_hash,
        block_details,
        accounts: HashMap::new(),
        transactions: HashMap::new(),
        contract_modules: HashSet::new(),
        contract_instances: HashMap::new(),
    };

    block_events
        .try_for_each(|be| {
            match be {
                BlockEvent::AccountCreation(address, details) => {
                    block_data.accounts.insert(address, details);
                }
                BlockEvent::AccountTransaction(hash, details) => {
                    block_data.transactions.insert(hash, details);
                }
                BlockEvent::ContractModuleDeployment(module_ref) => {
                    block_data.contract_modules.insert(module_ref);
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

/// Queries the node available at `Args.endpoint` from height received from DB
/// process for `Args.num_blocks` blocks. Sends the data structured by block to
/// DB process through `block_sender`.
async fn run_node_process(
    height_receiver: tokio::sync::oneshot::Receiver<AbsoluteBlockHeight>,
    block_sender: tokio::sync::mpsc::Sender<BlockData>,
) -> anyhow::Result<()> {
    let args = Args::parse();
    let endpoint = args.node;
    let from_height = height_receiver
        .await
        .context("Did not receive height of most recent block recorded in database")?;
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

    let start_height = if from_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            let genesis_block_data =
                process_genesis_block(&mut node, genesis_block.block_hash).await?;
            block_sender
                .send(BlockData::Genesis(genesis_block_data))
                .await?;
            log::info!("Processed genesis block: {}", genesis_block.block_hash);
        }
        from_height.height + 1
    } else {
        from_height.height
    };

    for height in start_height..blocks_to_process {
        if let Some(block) = blocks_stream.next().await {
            let block_data = process_block(&mut node, block.block_hash).await?;
            block_sender.send(BlockData::Normal(block_data)).await?;
            log::info!("Processed block ({}): {}", height, block.block_hash);
        }
    }

    Ok(())
}

/// Inserts the `block_data` collected for a single block into the database
/// defined by `db`. Everything is commited as a single transactions allowing
/// for easy restoration from the last recorded block (by height) inserted into
/// the database.
async fn db_insert_block(db: &mut DBConn, block_data: BlockData) -> anyhow::Result<()> {
    let tx = db
        .client
        .transaction()
        .await
        .context("Failed to build transaction")?;

    let tx_ref = &tx;
    let prepared_ref = &db.prepared;

    let insert_common = |block_hash: BlockHash, block_details: BlockDetails| async move {
        prepared_ref
            .insert_block(tx_ref, block_hash, block_details)
            .await
    };

    match block_data {
        BlockData::Genesis(GenesisBlockData {
            block_hash,
            block_details,
            ..
        }) => {
            insert_common(block_hash, block_details).await?;
        }
        BlockData::Normal(NormalBlockData {
            block_hash,
            block_details,
            ..
        }) => {
            insert_common(block_hash, block_details).await?;
        }
    }

    tx.commit().await.context("Failed to commit transaction.")?;

    Ok(())
}

/// Runs a process of inserting data coming in on `block_receiver` in a database
/// defined in [`Args.db_connection`]
async fn run_db_process<'a>(
    mut block_receiver: tokio::sync::mpsc::Receiver<BlockData>,
    height_sender: tokio::sync::oneshot::Sender<AbsoluteBlockHeight>,
) -> anyhow::Result<()> {
    let args = Args::parse();
    let mut db = DBConn::create(args.db_connection, true).await?;
    let latest_height = db
        .prepared
        .get_latest_height(&db.client)
        .await
        .context("Could not get best height from database")?
        .map_or(0.into(), |h| h);

    height_sender
        .send(latest_height)
        .map_err(|_| anyhow!("Best block height could not be sent to node process"))?;

    while let Some(block_data) = block_receiver.recv().await {
        db_insert_block(&mut db, block_data).await?;
        println!("Inserted block into db");
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    env_logger::Builder::new()
        .filter_module(module_path!(), args.log_level) // Only log the current module (main).
        .init();

    // Since the database connection is managed by the background task we use a
    // oneshot channel to get the height we should start querying at. First the
    // background database task is started which then sends the height over this
    // channel.
    let (height_sender, height_receiver) = tokio::sync::oneshot::channel();
    // Create a channel between the task querying the node and the task logging
    // transactions.
    let (block_sender, block_receiver) = tokio::sync::mpsc::channel(100);

    tokio::spawn(run_db_process(block_receiver, height_sender));

    run_node_process(height_receiver, block_sender)
        .await
        .context("Error happened while querying node.")?;

    Ok(())
}

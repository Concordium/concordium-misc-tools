use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    indexer,
    smart_contracts::common::{AccountAddress, Amount, ACCOUNT_ADDRESS_SIZE},
    types::{
        hashes::{BlockHash, TransactionHash},
        queries::BlockInfo,
        smart_contracts::{ModuleReference, WasmVersion},
        AbsoluteBlockHeight, AccountCreationDetails, AccountTransactionDetails,
        AccountTransactionEffects, BlockItemSummary,
        BlockItemSummaryDetails::{AccountCreation, AccountTransaction},
        ContractAddress, CredentialType, OpenStatus, ProtocolVersion, RewardsOverview,
        SpecialTransactionOutcome, TransactionType,
    },
    v2::{self, AccountIdentifier, BlockIdentifier, Client, Endpoint, RelativeBlockHeight},
};
use core::fmt;
use futures::{self, stream::FuturesUnordered, StreamExt, TryStreamExt};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    Arc,
};
use tokio::task::JoinHandle;
use tokio_postgres::{
    types::{ToSql, Type},
    NoTls,
};
use tonic::transport::ClientTlsConfig;

/// Command line configuration of the application.
#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoints are expected to point to concordium node grpc v2 API's.",
        default_value = "http://localhost:20001",
        env = "KPI_TRACKER_NODES",
        value_delimiter = ','
    )]
    node_endpoints: Vec<Endpoint>,
    /// Database connection string.
    #[arg(
        long = "db-connection",
        default_value = "host=localhost dbname=kpi-tracker user=postgres password=password \
                         port=5432",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "KPI_TRACKER_DB_CONNECTION"
    )]
    db_connection: tokio_postgres::config::Config,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Debug, env = "KPI_TRACKER_LOG_LEVEL")]
    log_level: log::LevelFilter,
    /// Number of parallel queries to run against node
    #[arg(
        long = "num-parallel",
        default_value_t = 1,
        help = "The number of parallel queries to run against a node. Only relevant to set to \
                something different than 1 when catching up.",
        env = "KPI_TRACKER_NUM_PARALLEL"
    )]
    num_parallel: u8,
    /// Maximum number of blocks to insert into the database at the same time.
    #[arg(
        long = "bulk-insert-max",
        default_value_t = 20,
        help = "The number of blocks to insert in bulk. This helps during catchup since database \
                transaction commit is a significant amount of time. Blocks will only be inserted \
                if they are pending in the queue.",
        env = "KPI_TRACKER_BULK_INSERT_MAX"
    )]
    bulk_insert_max: usize,
    /// Max amount of seconds a response from a node can fall behind before
    /// trying another.
    #[arg(
        long = "max-behind-seconds",
        default_value_t = 240,
        env = "KPI_TRACKER_MAX_BEHIND_SECONDS"
    )]
    max_behind_s: u32,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        AccountAddress(self.0).fmt(f)
    }
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
    height: AbsoluteBlockHeight,
    /// [`PaydayBlockData`] for the block. This is only recorded for "payday"
    /// blocks reflected by `Some`, where non payday blocks are reflected by
    /// `None`.
    payday_data: Option<PaydayBlockData>,
    /// Block hash of the genesis block
    block_hash: BlockHash,
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
    transaction_type: Option<TransactionType>,
    /// The cost of the transaction.
    cost: Amount,
    /// Whether the transaction failed or not.
    is_success: bool,
    /// Accounts affected by the transactions.
    affected_accounts: Vec<CanonicalAccountAddress>,
    /// Contracts affected by the transactions.
    affected_contracts: Vec<ContractAddress>,
}

/// Holds selected attributes of a contract instance created on chain.
#[derive(Debug)]
struct ContractInstanceDetails {
    /// Foreign key to the module used to instantiate the contract
    module_ref: ModuleReference,
    /// Version of the contract.
    version: WasmVersion,
}

/// List of (canonical) account address, account detail pairs
type Accounts = Vec<(CanonicalAccountAddress, AccountDetails)>;
/// List of transaction hash, transaction detail pairs
type AccountTransactions = Vec<(TransactionHash, TransactionDetails)>;
/// List of contract modules references
type ContractModules = Vec<ModuleReference>;
/// List of contract address, contract instance detail pairs
type ContractInstances = Vec<(ContractAddress, ContractInstanceDetails)>;

/// Model for data collected for genesis block (of the entire chain, not
/// subsequent ones from protocol updates)
#[derive(Debug)]
struct ChainGenesisBlockData {
    /// Block details of the genesis block
    block_details: BlockDetails,
    /// Accounts included in the genesis block
    accounts: Accounts,
}

/// Model for data collected for normal blocks
#[derive(Debug)]
struct NormalBlockData {
    /// Block details of the block
    block_details: BlockDetails,
    /// Accounts created in the block
    accounts: Accounts,
    /// Transactions included in the block
    transactions: AccountTransactions,
    /// Smart contract module deployments included in the block
    contract_modules: ContractModules,
    /// Smart contract instantiations included in the block
    contract_instances: ContractInstances,
}

/// Used for sending data to db process.
#[derive(Debug)]
enum BlockData {
    ChainGenesis(ChainGenesisBlockData),
    Normal(NormalBlockData),
}

/// The set of queries used to communicate with the postgres DB.
struct PreparedStatements {
    /// Insert block into DB
    insert_block: tokio_postgres::Statement,
    /// Insert payday into DB
    insert_payday: tokio_postgres::Statement,
    /// Insert account into DB
    insert_account: tokio_postgres::Statement,
    /// Insert contract module into DB
    insert_contract_module: tokio_postgres::Statement,
    /// Insert contract instance into DB
    insert_contract_instance: tokio_postgres::Statement,
    /// Insert transaction into DB
    insert_transaction: tokio_postgres::Statement,
    /// Get the latest recorded block height from the DB
    get_latest_height: tokio_postgres::Statement,
    /// Select single contract module ID by module ref
    contract_module_by_ref: tokio_postgres::Statement,
    /// Update `account_transactions` and `account_activeness` tables in a
    /// single statement.
    update_account_stats: tokio_postgres::Statement,
    /// Update `contract_transactions` and `contract_activeness` tables in a
    /// single statement.
    update_contract_stats: tokio_postgres::Statement,
}

impl PreparedStatements {
    /// Construct `PreparedStatements` using the supplied
    /// `tokio_postgres::Client`
    async fn new(client: &tokio_postgres::Client) -> Result<Self, tokio_postgres::Error> {
        let insert_block = client
            .prepare(
                "INSERT INTO blocks (hash, timestamp, height) VALUES ($1, $2, $3) RETURNING id",
            )
            .await?;
        let insert_payday = client
            .prepare(
                "INSERT INTO paydays (block, total_equity_capital, total_passively_delegated, \
                 total_actively_delegated, total_ccd, pool_reward, finalizer_reward,
                 foundation_reward, num_pools, num_open_pools, num_closed_pools, \
                 num_closed_for_new_pools, num_delegation_recipients, num_finalizers, \
                 num_delegators) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
                 $14, $15)",
            )
            .await?;
        let insert_account = client
            .prepare("INSERT INTO accounts (address, block, is_initial) VALUES ($1, $2, $3)")
            .await?;
        let insert_contract_module = client
            .prepare("INSERT INTO modules (ref, block) VALUES ($1, $2)")
            .await?;
        let insert_contract_instance = client
            .prepare(
                "INSERT INTO contracts (index, subindex, version, module, block) VALUES ($1, $2, \
                 $3, $4, $5)",
            )
            .await?;

        let insert_transaction = client
            .prepare(
                "INSERT INTO transactions (id, hash, block, cost, is_success, type) VALUES ($1, \
                 $2, $3, $4, $5, $6)",
            )
            .await?;

        let get_latest_height = client
            .prepare("SELECT blocks.height FROM blocks ORDER BY blocks.id DESC LIMIT 1")
            .await?;
        let update_account_stats = client
            .prepare_typed(
                "WITH id_table AS (SELECT accounts.id FROM accounts WHERE address=$1 LIMIT 1)
                   , at AS (INSERT INTO accounts_transactions (account, transaction)
                            VALUES ((SELECT id FROM id_table), $2))
             INSERT INTO account_activeness (account, time)
                    VALUES ((SELECT id FROM id_table), date_seconds($3))
                    ON CONFLICT DO NOTHING",
                &[Type::BYTEA, Type::INT8, Type::INT8],
            )
            .await?;

        let update_contract_stats = client
            .prepare_typed(
                "WITH id_table AS (SELECT contracts.id FROM contracts
                                   WHERE index=$1 AND subindex=$2 LIMIT 1),
                  ct AS (INSERT INTO contracts_transactions (contract, transaction)
                         VALUES ((SELECT id FROM id_table), $3))
             INSERT INTO contract_activeness (contract, time)
                    VALUES ((SELECT id FROM id_table), date_seconds($4))
                    ON CONFLICT DO NOTHING",
                &[Type::INT8, Type::INT8, Type::INT8],
            )
            .await?;

        let contract_module_by_ref = client
            .prepare("SELECT modules.id FROM modules WHERE ref=$1 LIMIT 1")
            .await?;

        Ok(Self {
            insert_block,
            insert_payday,
            insert_account,
            insert_contract_module,
            insert_contract_instance,
            insert_transaction,
            get_latest_height,
            contract_module_by_ref,
            update_account_stats,
            update_contract_stats,
        })
    }

    /// Add block to DB transaction `db_tx`.
    async fn insert_block(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_details: &BlockDetails,
    ) -> Result<i64, tokio_postgres::Error> {
        let block_hash = block_details.block_hash;
        let values: [&(dyn ToSql + Sync); 3] = [
            &block_hash.as_ref(),
            &block_details.block_time.timestamp(),
            &(block_details.height.height as i64),
        ];

        let now = tokio::time::Instant::now();
        let row = db_tx.query_one(&self.insert_block, &values).await?;
        let id = row.try_get::<_, i64>(0)?;
        log::trace!("Took {}ms to insert block.", now.elapsed().as_millis());

        if let Some(payday_data) = block_details.payday_data {
            let values: [&(dyn ToSql + Sync); 15] = [
                &id,
                &(payday_data.total_equity_capital as i64),
                &(payday_data.total_passively_delegated as i64),
                &(payday_data.total_actively_delegated as i64),
                &(payday_data.total_ccd as i64),
                &(payday_data.pool_reward as i64),
                &(payday_data.finalizer_reward as i64),
                &(payday_data.foundation_reward as i64),
                &(payday_data.pool_count as i64),
                &(payday_data.open_pool_count as i64),
                &(payday_data.closed_for_new_pool_count as i64),
                &(payday_data.closed_pool_count as i64),
                &(payday_data.delegation_recipient_count as i64),
                &(payday_data.finalizer_count as i64),
                &(payday_data.delegator_count as i64),
            ];
            db_tx.execute(&self.insert_payday, &values).await?;
        }

        Ok(id)
    }

    /// Add account to DB transaction `db_tx`.
    async fn insert_account(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_id: i64,
        account_address: CanonicalAccountAddress,
        account_details: &AccountDetails,
    ) -> Result<(), tokio_postgres::Error> {
        let values: [&(dyn ToSql + Sync); 3] = [
            &account_address.0.as_ref(),
            &block_id,
            &account_details.is_initial,
        ];

        db_tx.query_opt(&self.insert_account, &values).await?;

        Ok(())
    }

    /// Add contract module to DB transaction `db_tx`.
    async fn insert_contract_module(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_id: i64,
        module_ref: ModuleReference,
    ) -> Result<(), tokio_postgres::Error> {
        let values: [&(dyn ToSql + Sync); 2] = [&module_ref.as_ref(), &block_id];

        db_tx
            .query_opt(&self.insert_contract_module, &values)
            .await?;

        Ok(())
    }

    /// Add contract instance to DB transaction `db_tx`.
    async fn insert_contract_instance(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_id: i64,
        contract_address: ContractAddress,
        contract_details: &ContractInstanceDetails,
    ) -> Result<(), tokio_postgres::Error> {
        let module_ref = contract_details.module_ref.as_ref();
        // It is not too bad to do two queries here since new instance
        // creations will be rare.
        let numeric_version = match contract_details.version {
            WasmVersion::V0 => 0i16,
            WasmVersion::V1 => 1i16,
        };
        let row = db_tx
            .query_one(&self.contract_module_by_ref, &[&module_ref])
            .await?;
        let module_id = row.try_get::<_, i64>(0)?;

        let values: [&(dyn ToSql + Sync); 5] = [
            &(contract_address.index as i64),
            &(contract_address.subindex as i64),
            &numeric_version,
            &module_id,
            &block_id,
        ];

        db_tx
            .query_opt(&self.insert_contract_instance, &values)
            .await?;

        Ok(())
    }

    /// Add transaction to DB transaction `db_tx`. This also adds relations
    /// between the transaction and contracts/accounts.
    async fn insert_transactions(
        &self,
        tx_num: &mut i64,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_id: i64,
        txs: &[(TransactionHash, TransactionDetails)],
    ) -> Result<Vec<(i64, Vec<CanonicalAccountAddress>, Vec<ContractAddress>)>, tokio_postgres::Error>
    {
        let mut futs = Vec::with_capacity(txs.len());
        for (transaction_hash, transaction_details) in txs {
            *tx_num += 1;
            let transaction_cost = transaction_details.cost.micro_ccd() as i64;
            let transaction_type = transaction_details.transaction_type.map(|tt| tt as i16);
            let id = *tx_num;
            futs.push(async move {
                let values: [&(dyn ToSql + Sync); 6] = [
                    &id,
                    &transaction_hash.as_ref(),
                    &block_id,
                    &transaction_cost,
                    &transaction_details.is_success,
                    &transaction_type,
                ];
                db_tx.query(&self.insert_transaction, &values).await?;
                Ok::<_, tokio_postgres::Error>((
                    id,
                    transaction_details.affected_accounts.clone(),
                    transaction_details.affected_contracts.clone(),
                ))
            })
        }

        let now = tokio::time::Instant::now();
        let res = futs
            .into_iter()
            .collect::<FuturesUnordered<_>>()
            .try_collect()
            .await?;
        log::trace!("Inserted transactions in {}ms.", now.elapsed().as_millis());
        Ok(res)
    }

    /// Add transaction to DB transaction `db_tx`. This also adds relations
    /// between the transaction and contracts/accounts.
    async fn insert_transaction(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        block_time: i64,
        id: i64,
        affected_accounts: &[CanonicalAccountAddress],
        affected_contracts: &[ContractAddress],
    ) -> Result<(), tokio_postgres::Error> {
        let mut queries = Vec::with_capacity(affected_accounts.len());
        for account in affected_accounts {
            queries.push(self.insert_account_transaction_relation(db_tx, id, *account, block_time));
        }
        futures::future::try_join_all(queries).await?;

        let mut queries = Vec::with_capacity(affected_accounts.len());
        for contract in affected_contracts {
            queries
                .push(self.insert_contract_transaction_relation(db_tx, id, *contract, block_time));
        }
        futures::future::try_join_all(queries).await?;

        Ok(())
    }

    /// Add account-transaction relation to DB transaction `db_tx`.
    async fn insert_account_transaction_relation(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        transaction_id: i64,
        account_address: CanonicalAccountAddress,
        block_time: i64,
    ) -> Result<(), tokio_postgres::Error> {
        db_tx
            .query_opt(
                &self.update_account_stats,
                &[&account_address.0.as_ref(), &transaction_id, &block_time],
            )
            .await?;
        Ok(())
    }

    /// Add contract-transaction relation to DB transaction `db_tx`.
    async fn insert_contract_transaction_relation(
        &self,
        db_tx: &tokio_postgres::Transaction<'_>,
        transaction_id: i64,
        contract_address: ContractAddress,
        block_time: i64,
    ) -> Result<(), tokio_postgres::Error> {
        let params: [&(dyn ToSql + Sync); 4] = [
            &(contract_address.index as i64),
            &(contract_address.subindex as i64),
            &transaction_id,
            &block_time,
        ];
        let _ = db_tx
            .query_opt(&self.update_contract_stats, &params)
            .await?;
        Ok(())
    }

    /// Get the latest block height recorded in the DB.
    async fn get_latest_height(
        &self,
        db: &tokio_postgres::Client,
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

/// Holds [`tokio_postgres::Client`] to query the database and
/// [`PreparedStatements`] which can be executed with the client.
struct DBConn {
    client: tokio_postgres::Client,
    prepared: PreparedStatements,
    connection_handle: JoinHandle<()>,
}

impl DBConn {
    /// Create new `DBConn` from `tokio_postgres::config::Config`. If
    /// `try_create_tables` is true, database tables are created using
    /// `/resources/schema.sql`.
    async fn create(
        conn_string: tokio_postgres::config::Config,
        try_create_tables: bool,
    ) -> anyhow::Result<Self> {
        let (client, connection) = conn_string
            .connect(NoTls)
            .await
            .context("Could not create database connection")?;

        let connection_handle = tokio::spawn(async move {
            if let Err(e) = connection.await {
                log::error!("Connection error: {}", e);
            }
        });

        if try_create_tables {
            let create_statements = include_str!("../resources/schema.sql");
            client
                .batch_execute(create_statements)
                .await
                .context("Failed to execute create statements")?;
        }

        let prepared = PreparedStatements::new(&client).await?;
        let db_conn = DBConn {
            client,
            prepared,
            connection_handle,
        };

        Ok(db_conn)
    }
}

/// Events from individual transactions to store in the database.
enum BlockEvent {
    AccountCreation(CanonicalAccountAddress, AccountDetails),
    AccountTransaction(TransactionHash, TransactionDetails),
    ContractModuleDeployment(ModuleReference),
    ContractInstantiation(ContractAddress, ContractInstanceDetails),
}

/// Queries node for account info for the `account` given at the block
/// represented by the `block_hash`
fn account_details(account_creation_details: &AccountCreationDetails) -> AccountDetails {
    let is_initial = match account_creation_details.credential_type {
        CredentialType::Initial => true,
        CredentialType::Normal => false,
    };

    AccountDetails { is_initial }
}

/// Returns accounts on chain at the given `block_hash`
async fn accounts_in_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> v2::QueryResult<Vec<(CanonicalAccountAddress, AccountDetails)>> {
    let accounts = node.get_account_list(block_hash).await?.response;

    let accounts_details_map = accounts
        .then(|res| {
            let mut node = node.clone();

            async move {
                let account = res?;
                let account_info = node
                    .get_account_info(&AccountIdentifier::Address(account), block_hash)
                    .await?
                    .response;
                Ok::<_, v2::QueryError>((account, account_info))
            }
        })
        .map_ok(|(account, info)| {
            let is_initial =
                info.account_credentials
                    .get(&0.into())
                    .is_some_and(|cdi| match cdi.value {
                        AccountCredentialWithoutProofs::Initial { .. } => true,
                        AccountCredentialWithoutProofs::Normal { .. } => false,
                    });

            let canonical_account = CanonicalAccountAddress::from(account);
            let details = AccountDetails { is_initial };

            (canonical_account, details)
        })
        .try_collect()
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
                        version: data.contract_version,
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

/// Processes a block, represented by `block_hash` by querying the node for
/// entities present in the block state, updating the `db`. Should only be
/// used to process the chain's genesis block.
async fn process_chain_genesis_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> v2::QueryResult<ChainGenesisBlockData> {
    let block_info = node
        .get_block_info(AbsoluteBlockHeight::from(0))
        .await?
        .response;

    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
        payday_data: None,
        block_hash,
    };

    let accounts = accounts_in_block(node, block_hash).await?;
    let genesis_data = ChainGenesisBlockData {
        block_details,
        accounts,
    };

    Ok(genesis_data)
}

/// Contains data about a payday block, which is relevant for the reward period
/// that contains the block.
#[derive(Debug, Clone, Copy)]
struct PaydayBlockData {
    /// Total number of microCCD staked by bakers themselves.
    total_equity_capital: u64,
    /// Total number of microCCD delegated passivly.
    total_passively_delegated: u64,
    /// Total number of microCCD delegated to specific pools.
    total_actively_delegated: u64,
    /// Total microCCD in existence.
    total_ccd: u64,
    /// Reward in microCCD for the winning baker pool.
    pool_reward: u64,
    /// Reward in microCCD for the winning finalizer.
    finalizer_reward: u64,
    /// Reward in microCCD for the foundation.
    foundation_reward: u64,
    /// The number of pools in this reward period.
    pool_count: usize,
    /// The number of pools open for new delegators at the time of the payday
    /// block.
    open_pool_count: usize,
    /// The number of pools closed for new delegators at the time of the payday
    /// block.
    closed_for_new_pool_count: usize,
    /// The number of pools closed for all delegators at the time of the payday
    /// block.
    closed_pool_count: usize,
    /// The number of pools with delegated capital in this reward period.
    delegation_recipient_count: usize,
    /// The number of finalizers in this reward period.
    finalizer_count: usize,
    /// The number of delegators in this reward period.
    delegator_count: usize,
}

/// If block specified by `block_hash` is a payday block (also implies >=
/// protocol version 4), this returns [`PaydayBlockData`]. Otherwise, returns
/// `None`.
async fn process_payday_block(
    mut node: Client,
    max_concurrent: usize,
    block_info: &BlockInfo,
    special_events: Vec<SpecialTransactionOutcome>,
) -> v2::QueryResult<Option<PaydayBlockData>> {
    if block_info.protocol_version <= ProtocolVersion::P3 {
        return Ok(None);
    }
    let block_ident = BlockIdentifier::RelativeHeight(RelativeBlockHeight {
        genesis_index: block_info.genesis_index,
        height: block_info.era_block_height,
        restrict: true,
    });
    let start_time = tokio::time::Instant::now();
    // Handle special payday events
    let mut pool_reward = 0;
    let mut finalizer_reward = 0;
    let mut foundation_reward = 0;
    let mut is_payday_block = false;
    for event in special_events {
        match event {
            SpecialTransactionOutcome::PaydayFoundationReward {
                development_charge, ..
            } => {
                foundation_reward += development_charge.micro_ccd();
                is_payday_block = true;
            }
            SpecialTransactionOutcome::PaydayPoolReward {
                baker_reward,
                finalization_reward,
                ..
            } => {
                pool_reward += baker_reward.micro_ccd();
                finalizer_reward += finalization_reward.micro_ccd();
                is_payday_block = true;
            }
            SpecialTransactionOutcome::PaydayAccountReward { .. } => {
                is_payday_block = true;
            }
            _ => {}
        }
    }

    if !is_payday_block {
        // Block wasn't a payday block
        return Ok(None);
    }

    // Get total CCDs in existence
    let a = tokio::time::Instant::now();
    let tokenomics_info = node.get_tokenomics_info(block_ident).await?.response;
    log::debug!(
        "Took {} ms to query tokenomics info.",
        tokio::time::Instant::now().duration_since(a).as_millis()
    );
    let (RewardsOverview::V1 { common: data, .. } | RewardsOverview::V0 { data }) = tokenomics_info;
    let total_ccd = data.total_amount.micro_ccd();

    // Count entities and staked CCDs
    let a = tokio::time::Instant::now();
    let baker_response = node.get_bakers_reward_period(block_ident).await?.response;
    log::debug!(
        "Took {} ms to query bakers in reward period.",
        tokio::time::Instant::now().duration_since(a).as_millis()
    );

    let baker_list = node.get_baker_list(block_ident).await?.response;

    let pool_count = AtomicUsize::new(0);
    let open_pool_count = AtomicUsize::new(0);
    let closed_for_new_pool_count = AtomicUsize::new(0);
    let closed_pool_count = AtomicUsize::new(0);
    let delegation_recipient_count = AtomicUsize::new(0);
    let finalizer_count = AtomicUsize::new(0);
    let total_actively_delegated = AtomicU64::new(0);
    let total_equity_capital = AtomicU64::new(0);
    let delegating_account_count = AtomicUsize::new(0);

    let a = tokio::time::Instant::now();

    baker_response
        .map_err(Into::into)
        .try_for_each_concurrent(Some(max_concurrent), |baker| {
            let node = node.clone();
            async {
                // Clippy is not clever enough to see that this redefinition is necessary due to
                // ownership. The baker must be moved into this async block, and this is the way
                // to achieve it.
                #[allow(clippy::redundant_locals)]
                let baker = baker;
                let mut node = node;
                pool_count.fetch_add(1, Ordering::AcqRel);
                if baker.is_finalizer {
                    finalizer_count.fetch_add(1, Ordering::AcqRel);
                }
                total_actively_delegated
                    .fetch_add(baker.delegated_capital.micro_ccd(), Ordering::AcqRel);
                total_equity_capital.fetch_add(baker.equity_capital.micro_ccd(), Ordering::AcqRel);

                if baker.delegated_capital.micro_ccd() != 0 {
                    delegation_recipient_count.fetch_add(1, Ordering::AcqRel);
                    let delegators = node
                        .get_pool_delegators_reward_period(block_ident, baker.baker.baker_id)
                        .await?
                        .response;
                    delegating_account_count.fetch_add(delegators.count().await, Ordering::AcqRel);
                }

                Ok::<_, v2::QueryError>(())
            }
        })
        .await?;

    baker_list
        .map_err(Into::into)
        .try_for_each_concurrent(Some(max_concurrent), |baker_id| {
            let mut node = node.clone();
            let closed_for_new_pool_count = &closed_for_new_pool_count;
            let open_pool_count = &open_pool_count;
            let closed_pool_count = &closed_pool_count;
            async move {
                let pi = node.get_pool_info(block_ident, baker_id).await?.response;
                let pool_info = pi
                    .active_baker_pool_status
                    .expect("pool info missing")
                    .pool_info;
                match pool_info.open_status {
                    OpenStatus::OpenForAll => open_pool_count.fetch_add(1, Ordering::AcqRel),
                    OpenStatus::ClosedForNew => {
                        closed_for_new_pool_count.fetch_add(1, Ordering::AcqRel)
                    }
                    OpenStatus::ClosedForAll => closed_pool_count.fetch_add(1, Ordering::AcqRel),
                };
                Ok::<_, v2::QueryError>(())
            }
        })
        .await?;

    log::debug!(
        "Took {} ms to query baker lists.",
        tokio::time::Instant::now().duration_since(a).as_millis()
    );

    let a = tokio::time::Instant::now();
    let passive_delegators = node
        .get_passive_delegators_reward_period(block_ident)
        .await?
        .response;
    let delegator_count =
        delegating_account_count.load(Ordering::Acquire) + passive_delegators.count().await;
    log::debug!(
        "Took {} ms to query passive delegators lists.",
        tokio::time::Instant::now().duration_since(a).as_millis()
    );
    let total_passively_delegated = node
        .get_passive_delegation_info(block_ident)
        .await?
        .response
        .current_payday_delegated_capital
        .micro_ccd();

    let end = tokio::time::Instant::now();
    log::debug!(
        "Queried payday data for block {} in {}ms.",
        block_info.block_hash,
        end.duration_since(start_time).as_millis()
    );

    Ok(Some(PaydayBlockData {
        total_equity_capital: total_equity_capital.load(Ordering::Acquire),
        total_passively_delegated,
        total_actively_delegated: total_actively_delegated.load(Ordering::Acquire),
        total_ccd,
        pool_reward,
        finalizer_reward,
        foundation_reward,
        pool_count: pool_count.load(Ordering::Acquire),
        open_pool_count: open_pool_count.load(Ordering::Acquire),
        closed_for_new_pool_count: closed_for_new_pool_count.load(Ordering::Acquire),
        closed_pool_count: closed_pool_count.load(Ordering::Acquire),
        delegation_recipient_count: delegation_recipient_count.load(Ordering::Acquire),
        finalizer_count: finalizer_count.load(Ordering::Acquire),
        delegator_count,
    }))
}

struct KPIIndexer {
    inner: indexer::BlockEventsIndexer,
    max_concurrent: usize,
}

impl KPIIndexer {
    async fn get_block_data(
        &self,
        mut client: concordium_rust_sdk::v2::Client,
        fbi: concordium_rust_sdk::v2::FinalizedBlockInfo,
    ) -> concordium_rust_sdk::v2::QueryResult<BlockData> {
        use indexer::Indexer;
        // Genesis blocks are special
        if fbi.height == AbsoluteBlockHeight::from(0u64) {
            let genesis_block_data =
                process_chain_genesis_block(&mut client, fbi.block_hash).await?;
            return Ok(BlockData::ChainGenesis(genesis_block_data));
        }
        let client_clone = client.clone();
        let (bi, summaries, special) = self.inner.on_finalized(client_clone, &(), fbi).await?;

        let block_details = BlockDetails {
            block_time: bi.block_slot_time,
            height: bi.block_height,
            payday_data: process_payday_block(client, self.max_concurrent, &bi, special).await?,
            block_hash: bi.block_hash,
        };

        let mut block_data = NormalBlockData {
            block_details,
            accounts: Vec::new(),
            transactions: Vec::new(),
            contract_modules: Vec::new(),
            contract_instances: Vec::new(),
        };

        for bi_summary in summaries {
            for be in to_block_events(bi_summary) {
                match be {
                    BlockEvent::AccountCreation(address, details) => {
                        block_data.accounts.push((address, details));
                    }
                    BlockEvent::AccountTransaction(hash, details) => {
                        block_data.transactions.push((hash, details));
                    }
                    BlockEvent::ContractModuleDeployment(module_ref) => {
                        block_data.contract_modules.push(module_ref);
                    }
                    BlockEvent::ContractInstantiation(address, details) => {
                        block_data.contract_instances.push((address, details));
                    }
                };
            }
        }
        Ok(BlockData::Normal(block_data))
    }
}

#[indexer::async_trait]
impl indexer::Indexer for KPIIndexer {
    type Context = <indexer::BlockEventsIndexer as indexer::Indexer>::Context;
    type Data = BlockData;

    async fn on_connect<'a>(
        &mut self,
        endpoint: concordium_rust_sdk::v2::Endpoint,
        client: &'a mut concordium_rust_sdk::v2::Client,
    ) -> concordium_rust_sdk::v2::QueryResult<Self::Context> {
        self.inner.on_connect(endpoint, client).await
    }

    async fn on_finalized<'a>(
        &self,
        client: concordium_rust_sdk::v2::Client,
        _ctx: &'a Self::Context,
        fbi: concordium_rust_sdk::v2::FinalizedBlockInfo,
    ) -> concordium_rust_sdk::v2::QueryResult<Self::Data> {
        self.get_block_data(client, fbi).await
    }

    async fn on_failure(
        &mut self,
        endpoint: concordium_rust_sdk::v2::Endpoint,
        successive_failures: u64,
        err: indexer::TraverseError,
    ) -> bool {
        self.inner
            .on_failure(endpoint, successive_failures, err)
            .await
    }
}

/// Inserts the `block_datas` collected for one or more blocks into the database
/// defined by `db`. Everything is commited as a single transactions allowing
/// for easy restoration from the last recorded block (by height) inserted into
/// the database. Returns the heights of the processed block along with their
/// hashes and the duration it took to process the blocks.
async fn db_insert_blocks<'a, 'b>(
    db: &mut DBConn,
    block_datas: impl Iterator<Item = &'a BlockData>,
    tx_num: &mut i64,
) -> anyhow::Result<(
    Vec<(BlockHash, AbsoluteBlockHeight)>,
    usize,
    chrono::Duration,
)> {
    let start = chrono::Utc::now();
    let db_tx = db
        .client
        .transaction()
        .await
        .context("Failed to build DB transaction")?;

    let tx_ref = &db_tx;
    let prepared_ref = &db.prepared;

    let insert_common = |block_details: &'a BlockDetails, accounts: &'a Accounts| async move {
        let block_id = prepared_ref.insert_block(tx_ref, block_details).await?;

        for (address, details) in accounts.iter() {
            prepared_ref
                .insert_account(tx_ref, block_id, *address, details)
                .await?;
        }

        Ok::<_, tokio_postgres::Error>(block_id)
    };

    let mut heights = Vec::new();
    let mut num_txs = 0;
    for block_data in block_datas {
        match block_data {
            BlockData::ChainGenesis(ChainGenesisBlockData {
                block_details,
                accounts,
            }) => {
                let block_hash = block_details.block_hash;
                insert_common(block_details, accounts).await?;
                heights.push((block_hash, block_details.height));
            }
            BlockData::Normal(NormalBlockData {
                block_details,
                accounts,
                transactions,
                contract_modules,
                contract_instances,
            }) => {
                num_txs += transactions.len();
                let block_id = insert_common(block_details, accounts).await?;

                for module_ref in contract_modules.iter() {
                    db.prepared
                        .insert_contract_module(tx_ref, block_id, *module_ref)
                        .await?;
                }

                for (address, details) in contract_instances.iter() {
                    db.prepared
                        .insert_contract_instance(tx_ref, block_id, *address, details)
                        .await?;
                }

                let f = db
                    .prepared
                    .insert_transactions(tx_num, tx_ref, block_id, transactions)
                    .await?;

                let mut futs = Vec::new();
                let now = tokio::time::Instant::now();
                for (id, aff_accs, aff_contracts) in f.iter() {
                    futs.push(db.prepared.insert_transaction(
                        tx_ref,
                        block_details.block_time.timestamp(),
                        *id,
                        aff_accs,
                        aff_contracts,
                    ));
                }
                futures::future::try_join_all(futs).await?;
                log::trace!(
                    "Inserted all transactions in {}ms.",
                    now.elapsed().as_millis()
                );

                heights.push((block_details.block_hash, block_details.height));
            }
        }
    }

    let now = tokio::time::Instant::now();
    db_tx
        .commit()
        .await
        .context("Failed to commit DB transaction.")?;
    log::trace!("Commit completed in {}ms.", now.elapsed().as_millis());

    let end = chrono::Utc::now().signed_duration_since(start);
    Ok((heights, num_txs, end))
}

/// Runs a process of inserting data coming in on `block_receiver` in a database
/// defined in `db_connection`
async fn run_db_process(
    db_connection: tokio_postgres::config::Config,
    mut block_receiver: tokio::sync::mpsc::Receiver<BlockData>,
    height_sender: tokio::sync::oneshot::Sender<Option<AbsoluteBlockHeight>>,
    bulk_insert_max: usize,
    stop_flag: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    let mut db = DBConn::create(db_connection.clone(), true)
        .await
        .context("Could not create database connection")?;
    let latest_height = db
        .prepared
        .get_latest_height(&db.client)
        .await
        .context("Could not get best height from database")?;
    let tx_num = db
        .client
        .query_one("SELECT MAX(id) FROM transactions;", &[])
        .await
        .expect("OH");
    let mut tx_num = tx_num.try_get::<_, Option<i64>>(0)?.unwrap_or(0);

    height_sender
        .send(latest_height)
        .map_err(|_| anyhow!("Best block height could not be sent to node process"))?;

    // In case of DB errors, this is used to store the value to retry insertion for
    let mut retry_block_data = None;
    // How many successive insertion errors were encountered.
    // This is used to slow down attempts to not spam the database
    let mut successive_db_errors = 0;

    while !stop_flag.load(Ordering::Acquire) {
        let next_block_data = if retry_block_data.is_some() {
            retry_block_data
        } else {
            let v = block_receiver.recv().await;
            v.map(|x| vec![x])
        };

        if let Some(mut block_data) = next_block_data {
            // If there are pending blocks in the queue try to insert up to
            // `bulk_insert_max` of them in the same database transaction.
            {
                if block_data.len() < bulk_insert_max {
                    while let Ok(item) = block_receiver.try_recv() {
                        block_data.push(item);
                        if block_data.len() >= bulk_insert_max {
                            break;
                        }
                    }
                }
            }
            let checkpoint_tx_num = tx_num;
            match db_insert_blocks(&mut db, block_data.iter(), &mut tx_num).await {
                Ok((infos, num_txs, time)) => {
                    successive_db_errors = 0;
                    log::info!(
                        "Processed blocks {} at heights {} with {} transactions in {}ms",
                        infos
                            .iter()
                            .map(|x| x.0.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        infos
                            .iter()
                            .map(|x| x.1.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                        num_txs,
                        time.num_milliseconds()
                    );
                    retry_block_data = None;
                }
                Err(e) => {
                    tx_num = checkpoint_tx_num;
                    successive_db_errors += 1;
                    // wait for 2^(min(successive_errors - 1, 7)) seconds before attempting.
                    // The reason for the min is that we bound the time between reconnects.
                    let delay = std::time::Duration::from_millis(
                        500 * (1 << std::cmp::min(successive_db_errors, 8)),
                    );
                    log::error!(
                        "Database connection lost due to {:#}. Will attempt to reconnect in {}ms.",
                        e,
                        delay.as_millis()
                    );
                    tokio::time::sleep(delay).await;

                    let new_db = match DBConn::create(db_connection.clone(), false).await {
                        Ok(db) => db,
                        Err(e) => {
                            block_receiver.close();
                            return Err(e);
                        }
                    };

                    // and drop the old database connection.
                    let old_db = std::mem::replace(&mut db, new_db);
                    old_db.connection_handle.abort();

                    retry_block_data = Some(block_data);
                }
            }
        } else {
            break;
        }
    }

    block_receiver.close();
    db.connection_handle.abort();

    Ok(())
}
/// Construct a future for shutdown signals (for unix: SIGINT and SIGTERM) (for
/// windows: ctrl c and ctrl break). The signal handler is set when the future
/// is polled and until then the default signal handler.
async fn set_shutdown(flag: Arc<AtomicBool>) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use tokio::signal::unix as unix_signal;
        let mut terminate_stream = unix_signal::signal(unix_signal::SignalKind::terminate())?;
        let mut interrupt_stream = unix_signal::signal(unix_signal::SignalKind::interrupt())?;
        let terminate = Box::pin(terminate_stream.recv());
        let interrupt = Box::pin(interrupt_stream.recv());
        futures::future::select(terminate, interrupt).await;
        flag.store(true, Ordering::Release);
    }
    #[cfg(windows)]
    {
        use tokio::signal::windows as windows_signal;
        let mut ctrl_break_stream = windows_signal::ctrl_break()?;
        let mut ctrl_c_stream = windows_signal::ctrl_c()?;
        let ctrl_break = Box::pin(ctrl_break_stream.recv());
        let ctrl_c = Box::pin(ctrl_c_stream.recv());
        futures::future::select(ctrl_break, ctrl_c).await;
        flag.store(true, Ordering::Release);
    }
    Ok(())
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
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
    // node/db processes run until the stop flag is triggered.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let shutdown_handle = tokio::spawn(set_shutdown(stop_flag.clone()));

    let db_handle = tokio::spawn(run_db_process(
        args.db_connection,
        block_receiver,
        height_sender,
        args.bulk_insert_max,
        stop_flag.clone(),
    ));

    let latest_height = height_receiver
        .await
        .context("Did not receive height of most recent block recorded in database")?;

    let endpoints = args
        .node_endpoints
        .into_iter()
        .map(|node_endpoint| {
            // This uses whatever system certificates have been installed as trusted roots.
            if node_endpoint.uri().scheme() == Some(&v2::Scheme::HTTPS) {
                node_endpoint.tls_config(ClientTlsConfig::new())
            } else {
                Ok(node_endpoint)
            }
        })
        .collect::<Result<_, _>>()?;

    let kpi_indexer = KPIIndexer {
        inner: indexer::BlockEventsIndexer,
        max_concurrent: args.num_parallel.into(),
    };

    indexer::TraverseConfig::new(endpoints, latest_height.map_or(0.into(), |h| h.next()))
        .context("At least one endpoint must be provided.")?
        .set_max_parallel(args.num_parallel.into())
        .set_max_behind(std::time::Duration::from_secs(args.max_behind_s.into()))
        .traverse(kpi_indexer, block_sender)
        .await?;

    db_handle.abort();
    shutdown_handle.abort();
    Ok(())
}

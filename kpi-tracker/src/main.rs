use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::{
    id::types::AccountCredentialWithoutProofs,
    smart_contracts::common::{AccountAddress, Amount, ACCOUNT_ADDRESS_SIZE},
    types::{
        hashes::{BlockHash, TransactionHash},
        smart_contracts::ModuleReference,
        AbsoluteBlockHeight, AccountCreationDetails, AccountTransactionDetails,
        AccountTransactionEffects, BlockItemSummary,
        BlockItemSummaryDetails::{AccountCreation, AccountTransaction},
        ContractAddress, CredentialType, RewardsOverview, TransactionType,
    },
    v2::{AccountIdentifier, Client, Endpoint},
};
use core::fmt;
use futures::{self, future, stream::FuturesUnordered, Stream, StreamExt, TryStreamExt};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
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
    db_connection:  tokio_postgres::config::Config,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Debug, env = "KPI_TRACKER_LOG_LEVEL")]
    log_level:      log::LevelFilter,
    /// Number of parallel queries to run against node
    #[arg(
        long = "num-parallel",
        default_value_t = 1,
        help = "The number of parallel queries to run against a node. Only relevant to set to \
                something different than 1 when catching up.",
        env = "KPI_TRACKER_NUM_PARALLEL"
    )]
    num_parallel:   u8,
    /// Max amount of seconds a response from a node can fall behind before
    /// trying another.
    #[arg(
        long = "max-behind-seconds",
        default_value_t = 240,
        env = "KPI_TRACKER_MAX_BEHIND_SECONDS"
    )]
    max_behind_s:   u32,
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
    /// [`PaydayBlockData`] for the block. This is only recorded for "payday"
    /// blocks reflected by `Some`, where non payday blocks are reflected by
    /// `None`.
    payday_data: Option<PaydayBlockData>,
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
    module_ref: ModuleReference,
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
    /// Block hash of the genesis block
    block_hash:    BlockHash,
    /// Block details of the genesis block
    block_details: BlockDetails,
    /// Accounts included in the genesis block
    accounts:      Accounts,
}

/// Model for data collected for normal blocks
#[derive(Debug)]
struct NormalBlockData {
    /// Block hash of the block
    block_hash:         BlockHash,
    /// Block details of the block
    block_details:      BlockDetails,
    /// Accounts created in the block
    accounts:           Accounts,
    /// Transactions included in the block
    transactions:       AccountTransactions,
    /// Smart contract module deployments included in the block
    contract_modules:   ContractModules,
    /// Smart contract instantiations included in the block
    contract_instances: ContractInstances,
}

impl BlockData {
    pub fn block_hash(&self) -> BlockHash {
        match self {
            BlockData::ChainGenesis(gd) => gd.block_hash,
            BlockData::Normal(n) => n.block_hash,
        }
    }

    pub fn num_transactions(&self) -> usize {
        match self {
            BlockData::ChainGenesis(_) => 0,
            BlockData::Normal(n) => n.transactions.len(),
        }
    }
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
    insert_block:             tokio_postgres::Statement,
    /// Insert payday into DB
    insert_payday:            tokio_postgres::Statement,
    /// Insert account into DB
    insert_account:           tokio_postgres::Statement,
    /// Insert contract module into DB
    insert_contract_module:   tokio_postgres::Statement,
    /// Insert contract instance into DB
    insert_contract_instance: tokio_postgres::Statement,
    /// Insert transaction into DB
    insert_transaction:       tokio_postgres::Statement,
    /// Get the latest recorded block height from the DB
    get_latest_height:        tokio_postgres::Statement,
    /// Select single contract module ID by module ref
    contract_module_by_ref:   tokio_postgres::Statement,
    /// Update `account_transactions` and `account_activeness` tables in a
    /// single statement.
    update_account_stats:     tokio_postgres::Statement,
    /// Update `contract_transactions` and `contract_activeness` tables in a
    /// single statement.
    update_contract_stats:    tokio_postgres::Statement,
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
            .prepare("INSERT INTO paydays (block, total_stake, num_bakers) VALUES ($1, $2, $3)")
            .await?;
        let insert_account = client
            .prepare("INSERT INTO accounts (address, block, is_initial) VALUES ($1, $2, $3)")
            .await?;
        let insert_contract_module = client
            .prepare("INSERT INTO modules (ref, block) VALUES ($1, $2)")
            .await?;
        let insert_contract_instance = client
            .prepare(
                "INSERT INTO contracts (index, subindex, module, block) VALUES ($1, $2, $3, $4)",
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
    async fn insert_block<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        block_hash: BlockHash,
        block_details: &BlockDetails,
    ) -> Result<i64, tokio_postgres::Error> {
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
            let values: [&(dyn ToSql + Sync); 3] = [
                &id,
                &(payday_data.total_stake.micro_ccd() as i64),
                &payday_data.baker_count,
            ];
            db_tx.execute(&self.insert_payday, &values).await?;
        }

        Ok(id)
    }

    /// Add account to DB transaction `db_tx`.
    async fn insert_account<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
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
    async fn insert_contract_module<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
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
    async fn insert_contract_instance<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        block_id: i64,
        contract_address: ContractAddress,
        contract_details: &ContractInstanceDetails,
    ) -> Result<(), tokio_postgres::Error> {
        let module_ref = contract_details.module_ref.as_ref();
        // It is not too bad to do two queries here since new instance
        // creations will be rare.
        let row = db_tx
            .query_one(&self.contract_module_by_ref, &[&module_ref])
            .await?;
        let module_id = row.try_get::<_, i64>(0)?;

        let values: [&(dyn ToSql + Sync); 4] = [
            &(contract_address.index as i64),
            &(contract_address.subindex as i64),
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
    async fn insert_transactions<'a, 'b>(
        &'a self,
        tx_num: &mut i64,
        db_tx: &tokio_postgres::Transaction<'b>,
        block_id: i64,
        txs: &'b [(TransactionHash, TransactionDetails)],
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
    async fn insert_transaction<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
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
    async fn insert_account_transaction_relation<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        transaction_id: i64,
        account_address: CanonicalAccountAddress,
        block_time: i64,
    ) -> Result<(), tokio_postgres::Error> {
        db_tx
            .query_opt(&self.update_account_stats, &[
                &account_address.0.as_ref(),
                &transaction_id,
                &block_time,
            ])
            .await?;
        Ok(())
    }

    /// Add contract-transaction relation to DB transaction `db_tx`.
    async fn insert_contract_transaction_relation<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
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
    client:            tokio_postgres::Client,
    prepared:          PreparedStatements,
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
        CredentialType::Initial { .. } => true,
        CredentialType::Normal { .. } => false,
    };

    AccountDetails { is_initial }
}

/// Returns accounts on chain at the given `block_hash`
async fn accounts_in_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<Vec<(CanonicalAccountAddress, AccountDetails)>> {
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
        .map_ok(|(account, info)| {
            let is_initial = info
                .account_credentials
                .get(&0.into())
                .map_or(false, |cdi| match cdi.value {
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
/// used to process the chain's genesis block.
async fn process_chain_genesis_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<ChainGenesisBlockData> {
    let block_info = node
        .get_block_info(block_hash)
        .await
        .with_context(|| format!("Could not get block info for genesis block: {}", block_hash))?
        .response;

    let block_details = BlockDetails {
        block_time:  block_info.block_slot_time,
        height:      block_info.block_height,
        payday_data: None,
    };

    let accounts = accounts_in_block(node, block_hash).await?;
    let genesis_data = ChainGenesisBlockData {
        block_hash,
        block_details,
        accounts,
    };

    Ok(genesis_data)
}

#[derive(Debug, Clone, Copy)]
struct PaydayBlockData {
    total_stake: Amount,
    /// The amount of active bakers. Only for >= protocol version 6.
    baker_count: Option<i64>,
}

/// If block specified by `block_hash` is a payday block (also implies >=
/// protocol version 4), this returns [`PaydayBlockData`]. Otherwise, returns
/// `None`.
async fn process_payday_block(
    node: &mut Client,
    block_hash: BlockHash,
) -> anyhow::Result<Option<PaydayBlockData>> {
    let is_payday_block = node
        .is_payday_block(block_hash)
        .await
        .with_context(|| {
            format!("Could not assert whether block is payday block for: {block_hash}")
        })?
        .response;

    if !is_payday_block {
        return Ok(None);
    }

    let tokenomics_info = node
        .get_tokenomics_info(block_hash)
        .await
        .with_context(|| format!("Could not get tokenomics info for block: {block_hash}"))?
        .response;

    let RewardsOverview::V1 {
        total_staked_capital,
        ..
    } = tokenomics_info else {
        return Ok(None);
    };

    // TODO: Concurrently query for bakers and total stake
    let baker_count = match node.get_bakers_reward_period(block_hash).await {
        Ok(bakers) => Some(bakers.response.count().await as i64),
        // Error means protocol version < 6
        Err(_) => None,
    };

    Ok(Some(PaydayBlockData {
        total_stake: total_staked_capital,
        baker_count,
    }))
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

    let payday_data = process_payday_block(node, block_hash).await?;
    let block_details = BlockDetails {
        block_time: block_info.block_slot_time,
        height: block_info.block_height,
        payday_data,
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
        accounts: Vec::new(),
        transactions: Vec::new(),
        contract_modules: Vec::new(),
        contract_instances: Vec::new(),
    };

    block_events
        .try_for_each(|be| {
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

            future::ok(())
        })
        .await?;

    Ok(block_data)
}

/// Queries the node available at `node_endpoint` from `latest_height` until
/// stopped. Sends the data structured by block to DB process through
/// `block_sender`. Process runs until stopped or an error happens internally.
async fn node_process(
    node_endpoint: Endpoint,
    latest_height: &mut Option<AbsoluteBlockHeight>,
    block_sender: &tokio::sync::mpsc::Sender<BlockData>,
    num_parallel: u8,
    max_behind_s: u32,
    stop_flag: &AtomicBool,
) -> anyhow::Result<()> {
    let from_height = latest_height.map_or(0.into(), |h| h.next());

    log::info!(
        "Processing blocks from height {} using node {}",
        from_height,
        node_endpoint.uri()
    );

    // Use TLS if the URI scheme is HTTPS.
    // This uses whatever system certificates have been installed as trusted roots.
    let node_endpoint = if node_endpoint
        .uri()
        .scheme()
        .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
    {
        node_endpoint.tls_config(ClientTlsConfig::new())?
    } else {
        node_endpoint
    };

    let mut node = Client::new(node_endpoint.clone())
        .await
        .context("Could not connect to node.")?;
    let mut blocks_stream = node
        .get_finalized_blocks_from(from_height)
        .await
        .context("Error querying blocks")?;

    if from_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            let genesis_block_data =
                process_chain_genesis_block(&mut node, genesis_block.block_hash).await?;
            block_sender
                .send(BlockData::ChainGenesis(genesis_block_data))
                .await?;
            log::info!("Processed genesis block: {}", genesis_block.block_hash);
        }
    };

    let timeout = Duration::from_secs(max_behind_s.into());

    while !stop_flag.load(Ordering::Acquire) {
        let (has_error, chunks) = blocks_stream
            .next_chunk_timeout(num_parallel.into(), timeout)
            .await
            .with_context(|| format!("Timeout reached for node: {}", node_endpoint.uri()))?;

        for block in chunks {
            let block_data = process_block(&mut node, block.block_hash).await?;
            if block_sender
                .send(BlockData::Normal(block_data))
                .await
                .is_err()
            {
                log::error!("The database connection has been closed. Terminating node queries.");
                return Ok(());
            }

            *latest_height = Some(block.height);
        }

        if has_error {
            return Err(anyhow!("Finalized block stream dropped"));
        }
    }

    log::info!("Service stopped gracefully from exit signal.");
    Ok(())
}

/// Inserts the `block_data` collected for a single block into the database
/// defined by `db`. Everything is commited as a single transactions allowing
/// for easy restoration from the last recorded block (by height) inserted into
/// the database. Returns the height of the processed block along with the
/// duration it took to process.
async fn db_insert_block<'a>(
    db: &mut DBConn,
    block_data: &'a BlockData,
    tx_num: &mut i64,
) -> anyhow::Result<(AbsoluteBlockHeight, chrono::Duration)> {
    let start = chrono::Utc::now();
    let db_tx = db
        .client
        .transaction()
        .await
        .context("Failed to build DB transaction")?;

    let tx_ref = &db_tx;
    let prepared_ref = &db.prepared;

    let insert_common = |block_hash: BlockHash,
                         block_details: &'a BlockDetails,
                         accounts: &'a Accounts| async move {
        let block_id = prepared_ref
            .insert_block(tx_ref, block_hash, block_details)
            .await?;

        for (address, details) in accounts.iter() {
            prepared_ref
                .insert_account(tx_ref, block_id, *address, details)
                .await?;
        }

        Ok::<_, tokio_postgres::Error>(block_id)
    };

    let height = match block_data {
        BlockData::ChainGenesis(ChainGenesisBlockData {
            block_hash,
            block_details,
            accounts,
        }) => {
            insert_common(*block_hash, block_details, accounts).await?;
            block_details.height
        }
        BlockData::Normal(NormalBlockData {
            block_hash,
            block_details,
            accounts,
            transactions,
            contract_modules,
            contract_instances,
        }) => {
            let block_id = insert_common(*block_hash, block_details, accounts).await?;

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

            block_details.height
        }
    };

    let now = tokio::time::Instant::now();
    db_tx
        .commit()
        .await
        .context("Failed to commit DB transaction.")?;
    log::trace!("Commit completed in {}ms.", now.elapsed().as_millis());

    let end = chrono::Utc::now().signed_duration_since(start);
    Ok((height, end))
}

/// Runs a process of inserting data coming in on `block_receiver` in a database
/// defined in `db_connection`
async fn run_db_process(
    db_connection: tokio_postgres::config::Config,
    mut block_receiver: tokio::sync::mpsc::Receiver<BlockData>,
    height_sender: tokio::sync::oneshot::Sender<Option<AbsoluteBlockHeight>>,
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
            block_receiver.recv().await
        };

        if let Some(block_data) = next_block_data {
            let checkpoint_tx_num = tx_num;
            match db_insert_block(&mut db, &block_data, &mut tx_num).await {
                Ok((height, time)) => {
                    successive_db_errors = 0;
                    log::info!(
                        "Processed block {} at height {} with {} transactions in {}ms",
                        block_data.block_hash(),
                        height.height,
                        block_data.num_transactions(),
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
    // node/db processes run until the stop flag is triggered.
    let stop_flag = Arc::new(AtomicBool::new(false));
    let shutdown_handle = tokio::spawn(set_shutdown(stop_flag.clone()));

    let db_handle = tokio::spawn(run_db_process(
        args.db_connection,
        block_receiver,
        height_sender,
        stop_flag.clone(),
    ));

    let mut latest_height = height_receiver
        .await
        .context("Did not receive height of most recent block recorded in database")?;

    let mut latest_successful_node: u64 = 0;
    let num_nodes = args.node_endpoints.len() as u64;
    for (node, i) in args.node_endpoints.into_iter().cycle().zip(0u64..) {
        let start_height = latest_height;

        if stop_flag.load(Ordering::Acquire) {
            break;
        }

        if i.saturating_sub(latest_successful_node) >= num_nodes {
            // we skipped all the nodes without success.
            let delay = std::time::Duration::from_secs(5);
            log::error!(
                "Connections to all nodes have failed. Pausing for {}s before trying node {} \
                 again.",
                delay.as_secs(),
                node.uri()
            );
            tokio::time::sleep(delay).await;
        }

        // The process keeps running until stopped manually, or an error happens.
        let node_result = node_process(
            node.clone(),
            &mut latest_height,
            &block_sender,
            args.num_parallel,
            args.max_behind_s,
            stop_flag.as_ref(),
        )
        .await;

        if let Err(e) = node_result {
            log::warn!(
                "Endpoint {} failed with error {}. Trying next.",
                node.uri(),
                e
            );
        } else {
            // `node_process` terminated with `Ok`, meaning we should stop the service
            // entirely.
            break;
        }

        if latest_height > start_height {
            latest_successful_node = i;
        }
    }

    db_handle.abort();
    shutdown_handle.abort();
    Ok(())
}

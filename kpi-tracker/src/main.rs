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
use core::fmt;
use futures::{self, future, Stream, StreamExt, TryStreamExt};
use tokio_postgres::{types::ToSql, NoTls};

/// Command line configuration of the application.
#[derive(Debug, Parser)]
struct Args {
    /// The node used for querying
    #[arg(
        long = "node",
        help = "The endpoint is expected to point to a concordium node grpc v2 API.",
        default_value = "http://localhost:20001"
    )]
    node:          Endpoint,
    /// Database connection string.
    #[arg(
        long = "db-connection",
        default_value = "host=localhost dbname=kpi-tracker user=postgres password=password \
                         port=5432",
        help = "A connection string detailing the connection to the database used by the \
                application."
    )]
    db_connection: tokio_postgres::config::Config,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Debug)]
    log_level:     log::LevelFilter,
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

/// List of (canonical) account address, account detail pairs
type Accounts = Vec<(CanonicalAccountAddress, AccountDetails)>;
/// List of transaction hash, transaction detail pairs
type AccountTransactions = Vec<(TransactionHash, TransactionDetails)>;
/// List of contract modules references
type ContractModules = Vec<ModuleRef>;
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
    /// Insert account into DB
    insert_account: tokio_postgres::Statement,
    /// Insert contract module into DB
    insert_contract_module: tokio_postgres::Statement,
    /// Insert contract instance into DB
    insert_contract_instance: tokio_postgres::Statement,
    /// Insert transaction into DB
    insert_transaction: tokio_postgres::Statement,
    /// Insert account-transaction relation into DB
    insert_account_transaction_relation: tokio_postgres::Statement,
    /// Insert contract-transaction relation into DB
    insert_contract_transaction_relation: tokio_postgres::Statement,
    /// Get the latest recorded block height from the DB
    get_latest_height: tokio_postgres::Statement,
    /// Select single account ID by account address
    account_by_address: tokio_postgres::Statement,
    /// Select single cointract instance ID by contract address
    contract_by_address: tokio_postgres::Statement,
    /// Select single contract module ID by module ref
    contract_module_by_ref: tokio_postgres::Statement,
}

impl PreparedStatements {
    /// Construct `PreparedStatements` using the supplied
    /// `tokio_postgres::Client`
    async fn new(client: &tokio_postgres::Client) -> Result<Self, tokio_postgres::Error> {
        let insert_block = client
            .prepare(
                "INSERT INTO blocks (hash, timestamp, height, total_stake) VALUES ($1, $2, $3, \
                 $4) RETURNING id",
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
                "INSERT INTO contracts (index, subindex, module, block) VALUES ($1, $2, $3, $4)",
            )
            .await?;
        let insert_transaction = client
            .prepare(
                "INSERT INTO transactions (hash, block, cost, is_success, type) VALUES ($1, $2, \
                 $3, $4, $5) RETURNING id",
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
        let account_by_address = client
            .prepare("SELECT accounts.id FROM accounts WHERE address=$1 LIMIT 1")
            .await?;
        let contract_by_address = client
            .prepare("SELECT contracts.id FROM contracts WHERE index=$1 AND subindex=$2 LIMIT 1")
            .await?;
        let contract_module_by_ref = client
            .prepare("SELECT modules.id FROM modules WHERE ref=$1 LIMIT 1")
            .await?;

        Ok(Self {
            insert_block,
            insert_account,
            insert_contract_module,
            insert_contract_instance,
            insert_transaction,
            insert_account_transaction_relation,
            insert_contract_transaction_relation,
            get_latest_height,
            account_by_address,
            contract_by_address,
            contract_module_by_ref,
        })
    }

    /// Add block to DB transaction `db_tx`.
    async fn insert_block<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
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

        let row = db_tx.query_one(&self.insert_block, &values).await?;
        let id = row.try_get::<_, i64>(0)?;

        Ok(id)
    }

    /// Add account to DB transaction `db_tx`.
    async fn insert_account<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        block_id: i64,
        account_address: CanonicalAccountAddress,
        account_details: AccountDetails,
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
        module_ref: ModuleRef,
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
        contract_details: ContractInstanceDetails,
    ) -> Result<(), tokio_postgres::Error> {
        let module_ref = contract_details.module_ref.as_ref();
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
    async fn insert_transaction<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        block_id: i64,
        transaction_hash: TransactionHash,
        transaction_details: TransactionDetails,
    ) -> Result<(), tokio_postgres::Error> {
        let transaction_cost = transaction_details.cost.micro_ccd() as i64;
        let transaction_type = transaction_details.transaction_type.map(|tt| tt as i16);
        let values: [&(dyn ToSql + Sync); 5] = [
            &transaction_hash.as_ref(),
            &block_id,
            &transaction_cost,
            &transaction_details.is_success,
            &transaction_type,
        ];

        let row = db_tx.query_one(&self.insert_transaction, &values).await?;
        let id = row.try_get::<_, i64>(0)?;

        for account in transaction_details.affected_accounts {
            self.insert_account_transaction_relation(db_tx, id, account)
                .await?;
        }

        for contract in transaction_details.affected_contracts {
            self.insert_contract_transaction_relation(db_tx, id, contract)
                .await?;
        }

        Ok(())
    }

    /// Add account-transaction relation to DB transaction `db_tx`.
    async fn insert_account_transaction_relation<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        transaction_id: i64,
        account_address: CanonicalAccountAddress,
    ) -> Result<(), tokio_postgres::Error> {
        let row = db_tx
            .query_one(&self.account_by_address, &[&account_address.0.as_ref()])
            .await?;
        let account_id = row.try_get::<_, i64>(0)?;
        let values: [&(dyn ToSql + Sync); 2] = [&account_id, &transaction_id];

        db_tx
            .query_opt(&self.insert_account_transaction_relation, &values)
            .await?;

        Ok(())
    }

    /// Add contract-transaction relation to DB transaction `db_tx`.
    async fn insert_contract_transaction_relation<'a, 'b>(
        &'a self,
        db_tx: &tokio_postgres::Transaction<'b>,
        transaction_id: i64,
        contract_address: ContractAddress,
    ) -> Result<(), tokio_postgres::Error> {
        let contract_address: [&(dyn ToSql + Sync); 2] = [
            &(contract_address.index as i64),
            &(contract_address.subindex as i64),
        ];
        let row = db_tx
            .query_one(&self.contract_by_address, &contract_address)
            .await?;

        let contract_id = row.try_get::<_, i64>(0)?;
        let values: [&(dyn ToSql + Sync); 2] = [&contract_id, &transaction_id];

        db_tx
            .query_opt(&self.insert_contract_transaction_relation, &values)
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
    client:   tokio_postgres::Client,
    prepared: PreparedStatements,
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

        tokio::spawn(async move {
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
        total_stake: None,
    };

    let accounts = accounts_in_block(node, block_hash).await?;
    let genesis_data = ChainGenesisBlockData {
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

/// Queries the node available at `node_endpoint` from height received from DB
/// process until stopped. Sends the data structured by block to
/// DB process through `block_sender`.
async fn node_process(
    node_endpoint: Endpoint,
    latest_height: AbsoluteBlockHeight,
    block_sender: tokio::sync::mpsc::Sender<BlockData>,
) -> anyhow::Result<()> {
    log::info!(
        "Processing blocks from height {} using node {}",
        latest_height,
        node_endpoint.uri()
    );

    let mut node = Client::new(node_endpoint)
        .await
        .context("Could not connect to node.")?;

    let mut blocks_stream = node
        .get_finalized_blocks_from(latest_height)
        .await
        .context("Error querying blocks")?;

    if latest_height.height == 0 {
        if let Some(genesis_block) = blocks_stream.next().await {
            let genesis_block_data =
                process_chain_genesis_block(&mut node, genesis_block.block_hash).await?;
            block_sender
                .send(BlockData::ChainGenesis(genesis_block_data))
                .await?;
            log::info!("Processed genesis block: {}", genesis_block.block_hash);
        }
    };

    while let Some(block) = blocks_stream.next().await {
        let block_data = process_block(&mut node, block.block_hash).await?;
        block_sender.send(BlockData::Normal(block_data)).await?;
        log::info!(
            "Processed block ({}): {}",
            block.height.height,
            block.block_hash
        );
    }

    Ok(())
}

/// Inserts the `block_data` collected for a single block into the database
/// defined by `db`. Everything is commited as a single transactions allowing
/// for easy restoration from the last recorded block (by height) inserted into
/// the database.
async fn db_insert_block(db: &mut DBConn, block_data: BlockData) -> anyhow::Result<()> {
    let db_tx = db
        .client
        .transaction()
        .await
        .context("Failed to build DB transaction")?;

    let tx_ref = &db_tx;
    let prepared_ref = &db.prepared;

    let insert_common = |block_hash: BlockHash, block_details: BlockDetails, accounts: Accounts| async move {
        let block_id = prepared_ref
            .insert_block(tx_ref, block_hash, block_details)
            .await?;

        for (address, details) in accounts.into_iter() {
            prepared_ref
                .insert_account(tx_ref, block_id, address, details)
                .await?;
        }

        Ok::<_, tokio_postgres::Error>(block_id)
    };

    match block_data {
        BlockData::ChainGenesis(ChainGenesisBlockData {
            block_hash,
            block_details,
            accounts,
        }) => {
            insert_common(block_hash, block_details, accounts).await?;
        }
        BlockData::Normal(NormalBlockData {
            block_hash,
            block_details,
            accounts,
            transactions,
            contract_modules,
            contract_instances,
        }) => {
            let block_id = insert_common(block_hash, block_details, accounts).await?;

            for module_ref in contract_modules.into_iter() {
                db.prepared
                    .insert_contract_module(&db_tx, block_id, module_ref)
                    .await?;
            }

            for (address, details) in contract_instances.into_iter() {
                db.prepared
                    .insert_contract_instance(&db_tx, block_id, address, details)
                    .await?;
            }

            for (hash, details) in transactions.into_iter() {
                db.prepared
                    .insert_transaction(&db_tx, block_id, hash, details)
                    .await?;
            }
        }
    };

    db_tx
        .commit()
        .await
        .context("Failed to commit DB transaction.")?;
    Ok(())
}

/// Runs a process of inserting data coming in on `block_receiver` in a database
/// defined in `db_connection`
async fn run_db_process(
    db_connection: tokio_postgres::config::Config,
    mut block_receiver: tokio::sync::mpsc::Receiver<BlockData>,
    height_sender: tokio::sync::oneshot::Sender<AbsoluteBlockHeight>,
) -> anyhow::Result<()> {
    let mut db = DBConn::create(db_connection, true).await?;
    let latest_height = db
        .prepared
        .get_latest_height(&db.client)
        .await
        .context("Could not get best height from database")?
        .map_or(0.into(), |h| h.next());

    height_sender
        .send(latest_height)
        .map_err(|_| anyhow!("Best block height could not be sent to node process"))?;

    while let Some(block_data) = block_receiver.recv().await {
        db_insert_block(&mut db, block_data).await?;
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

    tokio::spawn(async {
        if let Err(e) = run_db_process(args.db_connection, block_receiver, height_sender).await {
            log::error!("Error happened while running DB process: {}", e);
        }
    });

    let latest_height = height_receiver
        .await
        .context("Did not receive height of most recent block recorded in database")?;

    node_process(args.node, latest_height, block_sender)
        .await
        .context("Error happened while querying node.")?;

    Ok(())
}

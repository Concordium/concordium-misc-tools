use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use anyhow::{anyhow, Context};
use chrono::{DateTime, Utc};
use clap::Parser;
use concordium_rust_sdk::smart_contracts::common::Amount;
use concordium_rust_sdk::types::hashes::TransactionHash;
use concordium_rust_sdk::types::smart_contracts::ModuleRef;
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

/// Holds selected attributes about accounts created on chain.
#[derive(Debug)]
struct AccountDetails {
    /// Whether an account was created as an initial account or not.
    is_initial: bool,
    /// FK to the block in which the account was created.
    block_hash: BlockHash,
}

/// Holds selected attributes of an account transaction.
#[derive(Debug)]
struct TransactionDetails {
    /// The transaction type of the account transaction
    transaction_type: TransactionType,
    /// FK to the block in which the transaction was finalized.
    block_hash: BlockHash,
    /// The cost of the transaction.
    cost: Amount,
}

/// Holds selected attributes of a contract module deployed on chain.
#[derive(Debug)]
struct ContractModuleDetails {
    /// FK to the block in which the module was deployed.
    block_hash: BlockHash,
}

/// Holds selected attributes of a contract instance created on chain.
#[derive(Debug)]
struct ContractInstanceDetails {
    /// FK to the module used to instantiate the contract
    module_ref: ModuleRef,
    /// FK to the block in which the contract was instantiated.
    block_hash: BlockHash,
}

/// Represents a compound unique constraint for relations between accounts and transactions
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
struct TransactionAccountRelation(AccountAddress, TransactionHash);

/// Represents a compound unique constraint for relations between contracts and transactions
#[derive(Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
struct TransactionContractRelation(ContractAddress, TransactionHash);

type BlocksTable = HashMap<BlockHash, BlockDetails>;
type AccountsTable = HashMap<AccountAddress, AccountDetails>;
type AccountTransactionsTable = HashMap<TransactionHash, TransactionDetails>;
type ContractModulesTable = HashMap<ModuleRef, ContractModuleDetails>;
type ContractInstancesTable = HashMap<ContractAddress, ContractInstanceDetails>;
type TransactionsAccountsTable = HashSet<TransactionAccountRelation>;
type TransactionsContractsTable = HashSet<TransactionContractRelation>;

/// This is intended as a in-memory DB, which follows the same schema as the final DB will follow.
struct DB {
    /// Table containing all blocks queried from node.
    blocks: BlocksTable,
    /// Table containing all accounts created on chain, along with accounts present in genesis
    /// block
    accounts: AccountsTable,
    /// Table containing all account transactions finalized on chain.
    account_transactions: AccountTransactionsTable,
    /// Table containing all smart contract modules deployed on chain.
    contract_modules: ContractModulesTable,
    /// Table containing all smart contract instances created on chain.
    contract_instances: ContractInstancesTable,
    /// Table containing relations between accounts and transactions.
    transaction_account_relations: TransactionsAccountsTable,
    /// Table containing relations between contract instances and transactions.
    transaction_contract_relations: TransactionsContractsTable,
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

/// Queries node for account info for the `account` given at the block represented by the
/// `block_hash`
fn account_details(
    block_hash: BlockHash,
    account_creation_details: Option<&AccountCreationDetails>,
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

/// Maps `AccountTransactionDetails` to `TransactionDetails`, where rejected transactions without a
/// transaction type are represented by `None`.
fn get_account_transaction_details(
    details: &AccountTransactionDetails,
    block_hash: BlockHash,
) -> Option<TransactionDetails> {
    details
        .transaction_type()
        .map(|transaction_type| TransactionDetails {
            block_hash,
            transaction_type,
            cost: details.cost,
        })
}

/// Maps `BlockItemSummary` to `BlockEvent`, which represent entities stored in the database.
fn to_block_events(block_hash: BlockHash, block_item: BlockItemSummary) -> Vec<BlockEvent> {
    let mut events: Vec<BlockEvent> = Vec::new();

    match &block_item.details {
        AccountTransaction(atd) => {
            // TODO: do we want to store failed transactions?
            if let Some(details) = get_account_transaction_details(atd, block_hash) {
                let affected_accounts = block_item
                    .affected_addresses()
                    .into_iter()
                    .map(|address| TransactionAccountRelation(address, block_item.hash));

                let affected_contracts = block_item
                    .affected_contracts()
                    .into_iter()
                    .map(|address| TransactionContractRelation(address, block_item.hash));

                println!(
                    "TRANSACTION:\nhash: {}\ndetails: {:?}",
                    block_item.hash, &details
                ); // Logger debug

                let event = BlockEvent::AccountTransaction(
                    block_item.hash,
                    details,
                    affected_accounts.collect(),
                    affected_contracts.collect(),
                );

                events.push(event);
            }

            match &atd.effects {
                AccountTransactionEffects::ModuleDeployed { module_ref } => {
                    let details = ContractModuleDetails { block_hash };
                    println!(
                        "CONTRACT MODULE:\nref: {}\ndetails: {:?}",
                        module_ref, &details
                    ); // Logger debug
                    let event = BlockEvent::ContractModuleDeployment(*module_ref, details);
                    events.push(event);
                }
                AccountTransactionEffects::ContractInitialized { data } => {
                    let details = ContractInstanceDetails {
                        block_hash,
                        module_ref: data.origin_ref,
                    };
                    println!(
                        "CONTRACT INSTANCE:\naddress: {}\ndetails: {:?}",
                        data.address, &details
                    ); // Logger debug
                    let event = BlockEvent::ContractInstantiation(data.address, details);
                    events.push(event);
                }
                _ => {}
            };
        }
        AccountCreation(act) => {
            let address = act.address;
            let details = account_details(block_hash, Some(act));

            println!("ACCOUNT:\naddress: {}\ndetails: {:?}", address, &details); // Logger debug

            let block_event = BlockEvent::AccountCreation(address, details);
            events.push(block_event);
        }
        _ => {}
    };

    events
}

/// Maps a stream of transactions to a stream of `BlockEvent`s
fn transactions_to_block_events<'a>(
    block_hash: &'a BlockHash,
    transactions_stream: impl Stream<Item = Result<BlockItemSummary, tonic::Status>> + 'a,
) -> impl Stream<Item = anyhow::Result<BlockEvent>> + 'a {
    transactions_stream.flat_map(move |res| {
        let block_events: Vec<Result<BlockEvent, anyhow::Error>> = match res {
            Ok(bi) => to_block_events(*block_hash, bi)
                .into_iter()
                .map(Ok)
                .collect(),
            Err(err) => vec![Err(anyhow!(
                "Error while streaming transactions for block  {}: {}",
                block_hash,
                err
            ))],
        };

        futures::stream::iter(block_events)
    })
}

/// Insert as a single DB transaction to facilitate easy recovery, as the service can restart from
/// current height stored in DB.
fn update_db(
    db: &mut DB,
    (block_hash, block_details): (BlockHash, &BlockDetails),
    accounts: BTreeMap<AccountAddress, AccountDetails>,
    transactions: Option<BTreeMap<TransactionHash, TransactionDetails>>,
    contract_modules: Option<BTreeMap<ModuleRef, ContractModuleDetails>>,
    contract_instances: Option<BTreeMap<ContractAddress, ContractInstanceDetails>>,
    transaction_account_relations: Option<BTreeSet<TransactionAccountRelation>>,
    transaction_contract_relations: Option<BTreeSet<TransactionContractRelation>>,
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
    if let Some(tars) = transaction_account_relations {
        db.transaction_account_relations.extend(tars.into_iter());
    }
    if let Some(tcrs) = transaction_contract_relations {
        db.transaction_contract_relations.extend(tcrs.into_iter());
    }
}

/// Processes a block, represented by `block_hash` by querying the node for entities present in the block state, updating the `db`. Should only be
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

    let accounts_in_block = node
        .get_account_list(block_hash)
        .await
        .context(format!("Could not get accounts for block: {}", block_hash))?
        .response;

    let genesis_accounts = accounts_in_block
        .try_fold(BTreeMap::new(), |mut map, account| async move {
            let details = account_details(block_hash, None);
            map.insert(account, details);
            Ok(map)
        })
        .await
        .context(format!(
            "Error while streaming accounts in block: {}",
            block_hash
        ))?;

    update_db(
        db,
        (block_hash, &block_details),
        genesis_accounts,
        None,
        None,
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
    let mut contract_modules: BTreeMap<ModuleRef, ContractModuleDetails> = BTreeMap::new();
    let mut contract_instances: BTreeMap<ContractAddress, ContractInstanceDetails> =
        BTreeMap::new();
    let mut transaction_account_relations: BTreeSet<TransactionAccountRelation> = BTreeSet::new();
    let mut transaction_contract_relations: BTreeSet<TransactionContractRelation> = BTreeSet::new();

    let block_events = transactions_to_block_events(&block_info.block_hash, transactions_stream);

    block_events
        .try_for_each(|be| {
            match be {
                BlockEvent::AccountCreation(address, details) => {
                    accounts.insert(address, details);
                }
                BlockEvent::AccountTransaction(
                    hash,
                    details,
                    affected_accounts,
                    affected_contracts,
                ) => {
                    account_transactions.insert(hash, details);

                    if !affected_accounts.is_empty() {
                        transaction_account_relations
                            .append(&mut BTreeSet::from_iter(affected_accounts.into_iter()));
                    }

                    if !affected_contracts.is_empty() {
                        transaction_contract_relations
                            .append(&mut BTreeSet::from_iter(affected_contracts.into_iter()));
                    }
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
        Some(transaction_account_relations),
        Some(transaction_contract_relations),
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

    // Print transaction-account relations
    let tar_strings: Vec<String> = db
        .transaction_account_relations
        .into_iter()
        .map(|TransactionAccountRelation(account, transaction)| {
            format!(
                "Transaction-Account relation: {} - {}",
                transaction, account
            )
        })
        .collect();

    println!(
        "{} transaction-account relations stored:\n{}\n",
        tar_strings.len(),
        tar_strings.join("\n")
    );

    // Print transaction-contract relations
    let tcr_strings: Vec<String> = db
        .transaction_contract_relations
        .into_iter()
        .map(|TransactionContractRelation(contract, transaction)| {
            format!(
                "Transaction-Contract relation: {} - {}",
                transaction, contract
            )
        })
        .collect();

    println!(
        "{} transaction-contract relations stored:\n{}\n",
        tcr_strings.len(),
        tcr_strings.join("\n")
    );
}

/// Queries the node available at `Args.endpoint` from `from_height` for `Args.num_blocks` blocks. Inserts
/// results captured into the supplied `db`.
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
    let mut db = DB {
        blocks: HashMap::new(),
        accounts: HashMap::new(),
        account_transactions: HashMap::new(),
        contract_modules: HashMap::new(),
        contract_instances: HashMap::new(),
        transaction_account_relations: HashSet::new(),
        transaction_contract_relations: HashSet::new(),
    };

    let current_height = AbsoluteBlockHeight { height: 0 }; // TOOD: get this from actual DB
    use_node(&mut db, current_height)
        .await
        .context("Error happened while querying node.")?;

    print_db(db);

    Ok(())
}

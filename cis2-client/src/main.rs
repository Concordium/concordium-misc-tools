use anyhow::{ensure, Context};
use clap::Parser;
use sha2::Digest;
use concordium_rust_sdk::{
    cis2::{
        self, AdditionalData, Cis2TransactionMetadata, OperatorUpdate, Receiver, TokenAmount,
        TokenId, Transfer,
    },
    common::types::{Amount, TransactionTime},
    types::{
        hashes::{BlockHash, TransactionHash, Hash},
        smart_contracts::OwnedReceiveName,
        transactions::send::GivenEnergy,
        Address, ContractAddress, WalletAccount,
    },
    v2::{self, BlockIdentifier},
};
use std::path::PathBuf;

/// The actions available for operators.
// Note that the arguments are duplicated here for better command-line
// ergonomics. Arguments without default values (such as `contract` cannot be
// marked `global` in `clap` at the moment, which leads to problems in usage
// since the arguments need to be given before the subcommand).
#[derive(clap::Subcommand, Debug)]
#[clap(arg_required_else_help(true))]
enum OperatorAction {
    #[clap(name = "add")]
    Add {
        #[clap(long = "operator", help = "Operator address.")]
        operator: Address,
        #[clap(
            long = "sender",
            help = "Sender of the transaction. This is the owner of the token."
        )]
        sender:   PathBuf,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
    },
    #[clap(name = "remove")]
    Remove {
        #[clap(long = "operator", help = "Operator address.")]
        operator: Address,
        #[clap(
            long = "sender",
            help = "Sender of the transaction. This is the owner of the token."
        )]
        sender:   PathBuf,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
    },
    #[clap(name = "query")]
    Query {
        #[clap(long = "operator", help = "Operator address.")]
        operator: Address,
        #[clap(long = "owner", help = "Owner address.")]
        owner:    Address,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
    },
}

#[derive(clap::Subcommand, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(author, version)]
#[clap(propagate_version = true)]
enum TransactionAction {
    #[clap(name = "show")]
    Show { hash: TransactionHash },
}

#[derive(clap::Subcommand, Debug)]
#[clap(author, version)]
#[clap(propagate_version = true)]
#[clap(arg_required_else_help(true))]
enum ContractAction {
    #[clap(name = "balance", about = "Query token balance for an address.")]
    Balance {
        #[clap(long = "address", help = "Address to query balance of.")]
        address:  Address,
        #[clap(long = "token", help = "Token id.")]
        token_id: TokenId,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
    },
    #[clap(name = "metadata", about = "Query token metadata.")]
    Metadata {
        #[clap(long = "token", help = "Token id.")]
        token_id: TokenId,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
    },
    #[clap(name = "operator", about = "Commands related to operators.")]
    OperatorOf {
        #[clap(subcommand)]
        action: OperatorAction,
    },
    #[clap(name = "transfer", about = "Transfer an amount of a token.")]
    Transfer {
        #[clap(long = "sender", help = "Sender of the transaction.")]
        sender:   PathBuf,
        #[clap(long = "token", help = "Token id.")]
        token_id: TokenId,
        #[clap(long = "contract", help = "Address of the token.")]
        contract: ContractAddress,
        #[clap(long = "amount", help = "Amount of token to transfer.")]
        amount:   TokenAmount,
        #[clap(
            long = "from",
            help = "Address to send from. Defaults to the address provided in the `sender` file."
        )]
        from:     Option<Address>,
        #[clap(long = "to", help = "Address to send to.")]
        to:       Address,
        #[clap(
            long = "notify",
            help = "The name of an entrypoint to invoke if transferring to a contract. Required \
                    if the `to` address is a contract address."
        )]
        notify:   Option<OwnedReceiveName>,
    },
}

#[derive(clap::Subcommand, Debug)]
#[clap(propagate_version = true)]
enum Action {
    #[clap(
        name = "contract",
        about = "Subcommands related to querying and updating the contract instance."
    )]
    Contract {
        #[clap(subcommand)]
        action: ContractAction,
    },
    #[clap(name = "transaction", about = "Commands to inspect CIS2 transactions.")]
    Transaction {
        #[clap(subcommand)]
        action: TransactionAction,
    },
}

#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(author, version, about)]
struct App {
    #[clap(
        long = "node",
        help = "GRPC V2 interface of the node.",
        default_value = "http://localhost:20000",
        global = true
    )]
    endpoint: v2::Endpoint,
    #[clap(
        long = "block",
        help = "Which block to query in. Defaults to last finalized block.",
        global = true
    )]
    block:    Option<BlockHash>,
    #[clap(subcommand)]
    command:  Action,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();
    let mut client = v2::Client::new(app.endpoint).await?;
    let app = App::parse();
    match app.command {
        Action::Contract { action } => handle_contract_command(client, app.block, action).await,
        Action::Transaction { action } => {
            match action {
                TransactionAction::Show { hash } => {
                    let status = client.get_block_item_status(&hash).await?;
                    if let Some((bh, summary)) = status.is_finalized() {
                        println!("Transaction is finalized in block {}.", bh);
                        if let Some(events) = summary.contract_update_logs() {
                            println!("Transaction cost of {}NRG.", summary.energy_cost);
                            println!("The following events were generated.");
                            for (ca, ca_events) in events {
                                println!("{}", ca);
                                for event in ca_events {
                                    match cis2::Event::try_from(event) {
                                        Ok(e) => println!("    - {}", e),
                                        Err(_) => {
                                            println!("    - Unparsable CIS2 event: {}", event)
                                        }
                                    }
                                }
                            }
                        } else if let Some(rr) = summary.is_rejected_account_transaction() {
                            println!("Transaction failed due to {:#?}.", rr);
                        } else {
                            anyhow::bail!(
                                "The transaction is not a smart contract update transaction."
                            )
                        }
                    } else {
                        println!("Transaction is not yet finalized.");
                    }
                }
            }
            Ok(())
        }
    }
}

async fn handle_contract_command(
    mut client: v2::Client,
    block: Option<BlockHash>,
    action: ContractAction,
) -> anyhow::Result<()> {
    let &contract = match &action {
        ContractAction::Balance { contract, .. } => contract,
        ContractAction::Metadata { contract, .. } => contract,
        ContractAction::OperatorOf { action } => match action {
            OperatorAction::Add { contract, .. } => contract,
            OperatorAction::Remove { contract, .. } => contract,
            OperatorAction::Query { contract, .. } => contract,
        },
        ContractAction::Transfer { contract, .. } => contract,
    };

    let contract_info_resp = client
        .get_instance_info(
            contract,
            block.map_or(BlockIdentifier::LastFinal, Into::into),
        )
        .await
        .context("Unable to inspect the contract instance.")?;
    let block = contract_info_resp.block_hash;
    let contract_info = contract_info_resp.response;
    let name = contract_info.name().clone();

    match action {
        ContractAction::Balance {
            address, token_id, ..
        } => {
            let mut cis2_client = cis2::Cis2Contract::new(client, contract, name);
            let balance = cis2_client
                .balance_of_single(&block, token_id.clone(), address)
                .await
                .context("Unable to get token balance.")?;
            println!(
                "Balance of token {} in contract {} for address {} is {}.",
                token_id,
                contract,
                address,
                balance.to_separated_string()
            );
        }
        ContractAction::Metadata { token_id, .. } => {
            let mut cis2_client = cis2::Cis2Contract::new(client, contract, name);
            let metadata_url = cis2_client
                .token_metadata_single(&block, token_id.clone())
                .await
                .context("Unable to get token metadata URL.")?;
            if let Some(hash) = metadata_url.hash() {
                println!(
                    "Metadata URL for token {} in contract {} is {} with hash {hash}",
                    token_id,
                    contract,
                    metadata_url.url()
                );
            } else {
                println!(
                    "Metadata URL for token {} in contract {} is {} without hash.",
                    token_id,
                    contract,
                    metadata_url.url()
                );
            };

            let resp = reqwest::get(metadata_url.url())
                .await
                .context("Cannot get metadata.")?;
            let bytes = resp.bytes().await?;
            let computed_hash: Hash = <[u8; 32]>::from(sha2::Sha256::digest(&bytes)).into();
            println!("Computed hash {computed_hash}");
            let r: serde_json::Value = serde_json::from_slice(&bytes)
                .context("Unable to parse metadata as JSON.")?;
            println!("{}", serde_json::to_string_pretty(&r)?)
        }
        ContractAction::OperatorOf { action } => {
            let mut cis2_client = cis2::Cis2Contract::new(client.clone(), contract, name);
            let (operator, sender, update) = match action {
                OperatorAction::Query {
                    operator, owner, ..
                } => {
                    let is_operator = cis2_client
                        .operator_of_single(&block, owner, operator)
                        .await
                        .context("Unable to get token balance.")?;
                    if is_operator {
                        println!("Address {} is an operator of {}.", operator, owner)
                    } else {
                        println!("Address {} is not an operator of {}.", operator, owner)
                    }
                    return Ok(());
                }
                OperatorAction::Add {
                    operator, sender, ..
                } => (operator, sender, OperatorUpdate::Add),
                OperatorAction::Remove {
                    operator, sender, ..
                } => (operator, sender, OperatorUpdate::Remove),
            };
            let keys = WalletAccount::from_json_file(sender)?;
            let energy = cis2_client
                .update_operator_single_dry_run(&block, keys.address.into(), operator, update)
                .await?;
            let expiry: TransactionTime =
                TransactionTime::from_seconds((chrono::Utc::now().timestamp() + 300) as u64);
            let nonce = client
                .get_next_account_sequence_number(&keys.address)
                .await?;
            ensure!(
                nonce.all_final,
                "There are non-finalized transactions. Aborting."
            );
            let metadata = Cis2TransactionMetadata {
                sender_address: keys.address,
                nonce: nonce.nonce,
                expiry,
                energy: GivenEnergy::Add(energy),
                amount: Amount::from_micro_ccd(0),
            };
            let tx_hash = cis2_client
                .update_operator_single(&keys, metadata, operator, update)
                .await?;
            println!("Submitted transaction {}.", tx_hash);
            let (block, summary) = client.wait_until_finalized(&tx_hash).await?;
            println!("Transaction finalized in block {}.", block);
            if let Some(reject_reason) = summary.is_rejected_account_transaction() {
                println!("Transaction failed. The reason is {:#?}", reject_reason);
            } else if let Some(events) = summary.contract_update_logs() {
                println!("Transaction is successful.");
                for (ca, ca_events) in events {
                    println!("{}", ca);
                    for event in ca_events {
                        match cis2::Event::try_from(event) {
                            Ok(e) => println!("    - {}", e),
                            Err(_) => println!("    - Unparsable CIS2 event: {}", event),
                        }
                    }
                }
            }
        }
        ContractAction::Transfer {
            sender,
            token_id,
            contract,
            amount,
            from,
            to,
            notify,
        } => {
            let mut cis2_client = cis2::Cis2Contract::new(client.clone(), contract, name);
            let keys = WalletAccount::from_json_file(sender)?;
            let sender = keys.address.into();
            let receiver = match to {
                Address::Account(a) => Receiver::Account(a),
                Address::Contract(ca) => Receiver::Contract(
                    ca,
                    notify.context(
                        "`notify` entrypoint is required if transferring to a contract instance.",
                    )?,
                ),
            };
            let transfer = Transfer {
                token_id,
                amount,
                from: from.unwrap_or(sender),
                to: receiver,
                data: AdditionalData::new(Vec::new()).unwrap(), // FIXME: Add option to supply
            };
            let energy = cis2_client
                .transfer_single_dry_run(&block, sender, transfer.clone())
                .await?;
            let expiry: TransactionTime =
                TransactionTime::from_seconds((chrono::Utc::now().timestamp() + 300) as u64);
            let nonce = client
                .get_next_account_sequence_number(&keys.address)
                .await?;
            ensure!(
                nonce.all_final,
                "There are non-finalized transactions. Aborting."
            );
            let metadata = Cis2TransactionMetadata {
                sender_address: keys.address,
                nonce: nonce.nonce,
                expiry,
                energy: GivenEnergy::Add(energy),
                amount: Amount::from_micro_ccd(0),
            };
            let tx_hash = cis2_client
                .transfer_single(&keys, metadata, transfer)
                .await?;
            println!("Submitted transaction {}.", tx_hash);
            let (block, summary) = client.wait_until_finalized(&tx_hash).await?;
            println!("Transaction finalized in block {}.", block);
            if let Some(reject_reason) = summary.is_rejected_account_transaction() {
                println!("Transaction failed. The reason is {:#?}", reject_reason);
            } else if let Some(events) = summary.contract_update_logs() {
                println!("Transaction is successful.");
                for (ca, ca_events) in events {
                    println!("{}", ca);
                    for event in ca_events {
                        match cis2::Event::try_from(event) {
                            Ok(e) => println!("    - {}", e),
                            Err(_) => println!("    - Unparsable CIS2 event: {}", event),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

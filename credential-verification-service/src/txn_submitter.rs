use crate::node_client::NodeClient;
use crate::types::ServerError;
use anyhow::Context;
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::types::{Nonce, RegisteredData, WalletAccount};
use concordium_rust_sdk::web3id::did::Network;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

/// Holds the service state in memory.
///
/// Note: A new instance of this struct is created whenever the service restarts.
#[derive(Debug, Clone)]
pub struct TransactionSubmitter {
    /// The client to interact with the node.
    node_client: Box<dyn NodeClient>,
    /// Account that signs transactions
    account: Arc<Mutex<AccountWithSequence>>,
    /// The number of seconds in the future when the anchor transactions should expiry.
    transaction_expiry_secs: u32,
}

impl TransactionSubmitter {
    pub async fn init(
        mut node_client: Box<dyn NodeClient>,
        account_keys: WalletAccount,
        transaction_expiry_secs: u32,
    ) -> Result<Self, ServerError> {
        let nonce = node_client
            .get_next_account_sequence_number(&account_keys.address)
            .await
            .context("get account sequence number")?;

        let account = AccountWithSequence {
            account_keys,
            nonce,
        };

        info!(
            "Transaction submitter configured with account {} and current account nonce: {}.",
            account.account_keys.address, nonce
        );

        Ok(Self {
            node_client,
            account: Arc::new(Mutex::new(account)),
            transaction_expiry_secs,
        })
    }

    pub async fn submit_register_data_txn(
        &self,
        register_data: RegisteredData,
    ) -> Result<TransactionHash, ServerError> {
        todo!()
    }
}

/// Account keys and local view of what the next account sequence number is
#[derive(Debug)]
pub struct AccountWithSequence {
    /// The key and address of the account submitting the anchor transactions on-chain.
    account_keys: WalletAccount,
    /// The current nonce of the account submitting the anchor transactions on-chain.
    nonce: Nonce,
}

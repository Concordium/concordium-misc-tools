use crate::node_client::NodeClient;
use crate::types::ServerError;
use anyhow::Context;
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::transactions::{BlockItem, send};
use concordium_rust_sdk::common::types::TransactionTime;
use concordium_rust_sdk::endpoints::RPCError;
use concordium_rust_sdk::types::{Nonce, RegisteredData, WalletAccount};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;
use tracing::{info, warn};

/// Submitter of transactions. Holds account keys and local view on account sequence number.
/// And the configuration parameters for transaction metadata.
#[derive(Debug, Clone)]
pub struct TransactionSubmitter {
    /// The client to interact with the node.
    node_client: Box<dyn NodeClient>,
    /// Account that signs transactions
    account: Arc<Mutex<AccountWithSequence>>,
    /// The number of seconds in the future when the anchor transactions should expiry.
    transaction_expiry_secs: u32,
    /// Timeout to acquire lock on account sequence number
    acquire_account_sequence_lock_timeout: Duration,
}

impl TransactionSubmitter {
    pub async fn init(
        mut node_client: Box<dyn NodeClient>,
        account_keys: WalletAccount,
        transaction_expiry_secs: u32,
        acquire_account_sequence_lock_timeout: Duration,
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
            acquire_account_sequence_lock_timeout,
        })
    }

    pub async fn submit_register_data_txn(
        &self,
        register_data: RegisteredData,
    ) -> Result<TransactionHash, ServerError> {
        // Lock local view on account sequence number. This is necessary
        // since it is possible that API requests come in parallel. The nonce is
        // increased by 1 and its lock is released after the transaction is submitted to
        // the blockchain.
        let mut account_guard = time::timeout(
            self.acquire_account_sequence_lock_timeout,
            self.account.lock(),
        )
        .await
        .context("timeout waiting for local account sequence lock")?;

        let mut node_client = self.node_client.clone();

        // Transaction should expire after some seconds.
        let expiry = TransactionTime::seconds_after(self.transaction_expiry_secs);

        let txn = send::register_data(
            &account_guard.account_keys,
            account_guard.account_keys.address,
            account_guard.nonce,
            expiry,
            register_data.clone(),
        );
        let block_item = BlockItem::AccountTransaction(txn);

        let send_block_item_result = node_client.send_block_item(&block_item).await;

        match send_block_item_result {
            Ok(txn_hash) => {
                // If the submission of the anchor transaction was successful,
                // increase the account_sequence_number tracked in this service.
                account_guard.nonce.next_mut();
                Ok(txn_hash)
            }
            Err(err) if err.is_account_sequence_number_error() => {
                // If the error is due to an account sequence number mismatch,
                // refresh the value in the state and try to resubmit the transaction.

                warn!(
                    "Unable to submit transaction on-chain successfully due to account nonce mismatch. Account nonce will be refreshed and transaction will be re-submitted: {}",
                    err
                );

                // Refresh nonce
                let node_view_on_nonce = node_client
                    .get_next_account_sequence_number(&account_guard.account_keys.address)
                    .await
                    .context("get next account sequence number")?;

                account_guard.nonce = node_view_on_nonce;

                info!("Refreshed account nonce successfully.");

                let txn = send::register_data(
                    &account_guard.account_keys,
                    account_guard.account_keys.address,
                    account_guard.nonce,
                    expiry,
                    register_data,
                );
                let block_item = BlockItem::AccountTransaction(txn);

                let txn_hash = node_client
                    .send_block_item(&block_item)
                    .await
                    .context("submit transaction")?;

                info!(
                    "Successfully submitted anchor transaction after the account nonce was refreshed."
                );

                // Increment nonce since submit was successful
                account_guard.nonce.next_mut();

                Ok(txn_hash)
            }
            Err(err) => Err(anyhow::Error::from(err)
                .context("submit transaction")
                .into()),
        }
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

trait RPCErrorExt {
    /// If the query error is account sequence mismatch error
    fn is_account_sequence_number_error(&self) -> bool;
}

impl RPCErrorExt for RPCError {
    fn is_account_sequence_number_error(&self) -> bool {
        match self {
            RPCError::CallError(err) => {
                err.message() == "Duplicate nonce" || err.message() == "Nonce too large"
            }
            _ => false,
        }
    }
}

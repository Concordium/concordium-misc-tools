use crate::node_client::NodeClient;
use crate::types::ServerError;
use anyhow::{Context, anyhow};
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::transactions::{BlockItem, send};
use concordium_rust_sdk::common::types::TransactionTime;
use concordium_rust_sdk::endpoints::RPCError;
use concordium_rust_sdk::types::{Nonce, RegisteredData, WalletAccount};
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{info, warn};

/// Submitter of transactions. Holds account keys and local view on account sequence number.
/// And the configuration parameters for transaction metadata.
#[derive(Debug, Clone)]
pub struct TransactionSubmitter {
    /// The client to interact with the node.
    node_client: Box<dyn NodeClient>,
    /// Account that signs transactions
    account: Arc<tokio::sync::Mutex<AccountWithSequence>>,
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
            "Transaction submitter initialized with account {} and current account nonce: {}.",
            account.account_keys.address, nonce
        );

        Ok(Self {
            node_client,
            account: Arc::new(tokio::sync::Mutex::new(account)),
            transaction_expiry_secs,
            acquire_account_sequence_lock_timeout,
        })
    }

    /// Submit a register data transaction with the given data. Returns the
    /// hash of the submitted transaction, if it was submitted successfully.
    pub async fn submit_register_data_txn(
        &self,
        register_data: RegisteredData,
    ) -> Result<TransactionHash, ServerError> {
        // Lock local view on account sequence number. This is necessary
        // since requests to submit transaction are processed concurrently, but
        // "using" a sequence number and submitting the transaction must be done sequentially.
        // The nonce is increased by 1 and its lock is released after the transaction is submitted to
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
                    .context("submit transaction, second try with updated nonce")?;

                info!(
                    "Successfully submitted anchor transaction after the account nonce was refreshed."
                );

                // Increment nonce since submit was successful
                account_guard.nonce.next_mut();

                Ok(txn_hash)
            }
            Err(err) => Err(anyhow!(err).context("submit transaction").into()),
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

#[cfg(test)]
mod test {
    use crate::node_client::{AccountCredentials, NodeClient};
    use crate::txn_submitter::TransactionSubmitter;
    use chrono::{DateTime, Utc};
    use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
    use concordium_rust_sdk::base::transactions::{BlockItem, EncodedPayload, Payload};
    use concordium_rust_sdk::common::types::AccountAddress;
    use concordium_rust_sdk::endpoints::{QueryResult, RPCError, RPCResult};
    use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
    use concordium_rust_sdk::id::types::{ArInfo, GlobalContext, IpInfo};
    use concordium_rust_sdk::types::{
        CredentialRegistrationID, Nonce, TransactionStatus, WalletAccount,
    };
    use concordium_rust_sdk::v2::BlockIdentifier;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;
    use tonic::Status;

    /// Register data that makes mock fail when calling send_block_item
    const SUBMIT_FAIL_DATA: [u8; 10] = [0x0fu8; 10];

    async fn submitter(node_mock: NodeClientMock) -> TransactionSubmitter {
        let wallet_keys =
            WalletAccount::from_json_file("tests/dummyaccount.json").expect("dummyaccount");
        TransactionSubmitter::init(
            Box::new(node_mock),
            wallet_keys,
            10,
            Duration::from_millis(1000),
        )
        .await
        .expect("init submitter")
    }

    /// Test submit transactions. Tests nonce management.
    #[tokio::test]
    async fn test_submit_transactions() {
        let mut node_mock = NodeClientMock::new(3.into());
        let submitter = submitter(node_mock.clone()).await;

        // assert nonce is as given by node
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(3));

        // submit transaction
        let txn_hash = submitter
            .submit_register_data_txn(vec![0, 1, 2].try_into().unwrap())
            .await
            .expect("submit");

        // assert nonce is now one higher
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(4));
        // assert transaction was submitted to node
        node_mock.expect_send_block_item(&txn_hash);

        // submit second transaction
        let txn_hash = submitter
            .submit_register_data_txn(vec![0, 1, 3].try_into().unwrap())
            .await
            .expect("submit");

        // assert nonce is now one higher
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(5));
        // assert transaction was submitted to node
        node_mock.expect_send_block_item(&txn_hash);
    }

    /// Test that if nonce is out of sync, the submitter will fetch nonce from node and adjust.
    #[tokio::test]
    async fn test_nonce_out_of_sync_too_low() {
        let mut node_mock = NodeClientMock::new(3.into());
        let submitter = submitter(node_mock.clone()).await;

        // set local nonce too low
        submitter.account.lock().await.nonce = Nonce::from(2);

        // submit transaction
        let txn_hash = submitter
            .submit_register_data_txn(vec![0, 1, 2].try_into().unwrap())
            .await
            .expect("submit");

        // assert nonce is now adjusted
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(4));
        // assert transaction was submitted to node
        node_mock.expect_send_block_item(&txn_hash);
    }

    /// Test that if nonce is out of sync, the submitter will fetch nonce from node and adjust.
    #[tokio::test]
    async fn test_nonce_out_of_sync_too_high() {
        let mut node_mock = NodeClientMock::new(3.into());
        let submitter = submitter(node_mock.clone()).await;

        // set local nonce too high
        submitter.account.lock().await.nonce = Nonce::from(4);

        // submit transaction
        let txn_hash = submitter
            .submit_register_data_txn(vec![0, 1, 2].try_into().unwrap())
            .await
            .expect("submit");

        // assert nonce is now adjusted
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(4));
        // assert transaction was submitted to node
        node_mock.expect_send_block_item(&txn_hash);
    }

    /// Test that if submit fails, nonce is not incremented
    #[tokio::test]
    async fn test_submit_fail() {
        let node_mock = NodeClientMock::new(3.into());
        let submitter = submitter(node_mock.clone()).await;

        // submit transaction
        submitter
            .submit_register_data_txn(SUBMIT_FAIL_DATA.to_vec().try_into().unwrap())
            .await
            .expect_err("submit");

        // assert nonce is not incremented
        assert_eq!(submitter.account.lock().await.nonce, Nonce::from(3));
    }

    /// Stub implementation of the node interface
    #[derive(Debug, Clone)]
    struct NodeClientMock(Arc<parking_lot::Mutex<NodeClientMockInner>>);

    impl NodeClientMock {
        fn new(nonce: Nonce) -> Self {
            let inner = NodeClientMockInner {
                account_sequence_number: nonce,
                send_block_items: Default::default(),
            };

            Self(Arc::new(parking_lot::Mutex::new(inner)))
        }
    }

    impl NodeClientMock {
        fn expect_send_block_item(
            &mut self,
            txn_hash: &TransactionHash,
        ) -> BlockItem<EncodedPayload> {
            self.0
                .lock()
                .send_block_items
                .remove(txn_hash)
                .expect("expected send block item")
        }
    }

    #[derive(Debug)]
    pub struct NodeClientMockInner {
        account_sequence_number: Nonce,
        send_block_items: HashMap<TransactionHash, BlockItem<EncodedPayload>>,
    }

    #[async_trait::async_trait]
    impl NodeClient for NodeClientMock {
        async fn get_next_account_sequence_number(
            &mut self,
            _address: &AccountAddress,
        ) -> QueryResult<Nonce> {
            Ok(self.0.lock().account_sequence_number)
        }

        async fn send_block_item(
            &mut self,
            bi: &BlockItem<EncodedPayload>,
        ) -> RPCResult<TransactionHash> {
            let txn_hash = bi.hash();
            if let BlockItem::AccountTransaction(txn) = bi {
                #[allow(clippy::comparison_chain)]
                if txn.header.nonce > self.0.lock().account_sequence_number {
                    return Err(RPCError::CallError(Status::invalid_argument(
                        "Nonce too large",
                    )));
                } else if txn.header.nonce < self.0.lock().account_sequence_number {
                    return Err(RPCError::CallError(Status::invalid_argument(
                        "Duplicate nonce",
                    )));
                }

                if let Payload::RegisterData { data } = txn.payload.decode().unwrap() {
                    if data.as_ref() == &SUBMIT_FAIL_DATA {
                        return Err(RPCError::CallError(Status::internal("Failing for test")));
                    }
                }

                self.0.lock().account_sequence_number.next_mut();
            }
            self.0.lock().send_block_items.insert(txn_hash, bi.clone());
            Ok(txn_hash)
        }

        async fn get_genesis_block_hash(&mut self) -> QueryResult<BlockHash> {
            unimplemented!()
        }

        async fn get_cryptographic_parameters(
            &mut self,
            _bi: BlockIdentifier,
        ) -> QueryResult<GlobalContext<ArCurve>> {
            unimplemented!()
        }

        async fn get_block_slot_time(
            &mut self,
            _bi: BlockIdentifier,
        ) -> QueryResult<DateTime<Utc>> {
            unimplemented!()
        }

        async fn get_block_item_status(
            &mut self,
            _th: &TransactionHash,
        ) -> QueryResult<TransactionStatus> {
            unimplemented!()
        }

        async fn get_account_credentials(
            &mut self,
            _cred_id: CredentialRegistrationID,
            _bi: BlockIdentifier,
        ) -> QueryResult<AccountCredentials> {
            unimplemented!()
        }

        async fn get_identity_providers(
            &mut self,
            _bi: BlockIdentifier,
        ) -> QueryResult<Vec<IpInfo<IpPairing>>> {
            unimplemented!()
        }

        async fn get_anonymity_revokers(
            &mut self,
            _bi: BlockIdentifier,
        ) -> QueryResult<Vec<ArInfo<ArCurve>>> {
            unimplemented!()
        }

        fn box_clone(&self) -> Box<dyn NodeClient> {
            Box::new(self.clone())
        }
    }
}

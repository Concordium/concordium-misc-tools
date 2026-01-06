use crate::integration_test_helpers::fixtures;
use crate::integration_test_helpers::fixtures::chain::GENESIS_BLOCK_HASH;
use chrono::{DateTime, Utc};
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
use concordium_rust_sdk::base::transactions::{BlockItem, EncodedPayload};
use concordium_rust_sdk::common;
use concordium_rust_sdk::endpoints::{QueryResult, RPCResult};
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::types::{ArInfo, GlobalContext, IpInfo};
use concordium_rust_sdk::types::{
    BlockItemSummary, CredentialRegistrationID, Nonce, TransactionStatus,
};
use concordium_rust_sdk::v2::{BlockIdentifier, QueryError, RPCError};
use credential_verification_service::node_client::{AccountCredentials, NodeClient};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::Status;

/// Return mock implementation of the node interface
pub fn node_client(global_context: GlobalContext<ArCurve>) -> NodeClientMock {
    let inner = NodeClientMockInner {
        ars: fixtures::credentials::ars(&global_context)
            .0
            .into_values()
            .collect(),
        ips: vec![fixtures::credentials::ip().public_ip_info],
        global_context,
        account_sequence_number: 1.into(),
        genesis_block_hash: fixtures::chain::GENESIS_BLOCK_HASH.into(),
        block_slot_time: DateTime::parse_from_rfc3339("2024-06-01T12:34:56Z")
            .unwrap()
            .to_utc(),
        block_item_statuses: Default::default(),
        account_credentials: Default::default(),
        send_block_items: Default::default(),
    };

    NodeClientMock(Arc::new(Mutex::new(inner)))
}

/// Mock implementation of the node interface
#[derive(Debug, Clone)]
pub struct NodeClientMock(Arc<Mutex<NodeClientMockInner>>);

impl NodeClientMock {
    pub fn stub_block_item_status(&self, txn_hash: TransactionHash, summary: TransactionStatus) {
        self.0.lock().block_item_statuses.insert(txn_hash, summary);
    }

    pub fn stub_account_credentials(
        &self,
        cred_id: CredentialRegistrationID,
        credentials: AccountCredentials,
    ) {
        let cred_id_bytes = common::to_bytes(&cred_id);
        self.0
            .lock()
            .account_credentials
            .insert(cred_id_bytes, credentials);
    }

    pub fn expect_send_block_item(&self, txn_hash: &TransactionHash) -> BlockItem<EncodedPayload> {
        self.0
            .lock()
            .send_block_items
            .remove(txn_hash)
            .expect("expected send block item")
    }
}

#[derive(Debug)]
pub struct NodeClientMockInner {
    global_context: GlobalContext<ArCurve>,
    ips: Vec<IpInfo<IpPairing>>,
    ars: Vec<ArInfo<ArCurve>>,
    account_sequence_number: Nonce,
    genesis_block_hash: BlockHash,
    block_slot_time: DateTime<Utc>,
    block_item_statuses: HashMap<TransactionHash, TransactionStatus>,
    account_credentials: HashMap<Vec<u8>, AccountCredentials>,
    send_block_items: HashMap<TransactionHash, BlockItem<EncodedPayload>>,
}

/// Clone TransactionStatus. Function can be removed when Clone is implemented on TransactionStatus in base.
fn clone_transaction_status(txn_status: &TransactionStatus) -> TransactionStatus {
    match txn_status {
        TransactionStatus::Received => TransactionStatus::Received,
        TransactionStatus::Finalized(val) => TransactionStatus::Finalized(val.clone()),
        TransactionStatus::Committed(val) => TransactionStatus::Committed(val.clone()),
    }
}

/// Transaction hash that makes mock fail when get_block_item_status is called with this hash
pub const GET_BLOCK_ITEM_FAIL_TXN_HASH: [u8; 32] = [0x0fu8; 32];

#[async_trait::async_trait]
impl NodeClient for NodeClientMock {
    async fn get_next_account_sequence_number(
        &mut self,
        _address: &AccountAddress,
    ) -> QueryResult<Nonce> {
        Ok(self.0.lock().account_sequence_number)
    }

    async fn get_genesis_block_hash(&mut self) -> QueryResult<BlockHash> {
        Ok(self.0.lock().genesis_block_hash)
    }

    async fn send_block_item(
        &mut self,
        bi: &BlockItem<EncodedPayload>,
    ) -> RPCResult<TransactionHash> {
        let txn_hash = bi.hash();
        let mut inner = self.0.lock();
        if let BlockItem::AccountTransaction(txn) = bi {
            if txn.header.nonce != inner.account_sequence_number {
                return Err(call_error(format!(
                    "account transaction sequence number {} does not match sequence number expected by stub: {}",
                    txn.header.nonce, inner.account_sequence_number
                )));
            }
            inner.account_sequence_number.next_mut();
        }
        inner.send_block_items.insert(txn_hash, bi.clone());
        Ok(txn_hash)
    }

    async fn get_cryptographic_parameters(
        &mut self,
        _bi: BlockIdentifier,
    ) -> QueryResult<GlobalContext<ArCurve>> {
        Ok(self.0.lock().global_context.clone())
    }

    async fn get_block_slot_time(&mut self, _bi: BlockIdentifier) -> QueryResult<DateTime<Utc>> {
        Ok(self.0.lock().block_slot_time)
    }

    async fn get_block_item_status(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<TransactionStatus> {
        if TransactionHash::from(GET_BLOCK_ITEM_FAIL_TXN_HASH) == *th {
            return Err(QueryError::RPCError(RPCError::CallError(Status::internal(
                "fail for test",
            ))));
        }

        self.0
            .lock()
            .block_item_statuses
            .get(th)
            .map(clone_transaction_status)
            .ok_or_else(|| {
                QueryError::RPCError(RPCError::CallError(Status::not_found("not found")))
            })
    }

    async fn wait_until_finalized(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<(BlockHash, BlockItemSummary)> {
        if TransactionHash::from(GET_BLOCK_ITEM_FAIL_TXN_HASH) == *th {
            return Err(QueryError::RPCError(RPCError::CallError(Status::internal(
                "fail for test",
            ))));
        }

        let txn_status = self
            .0
            .lock()
            .block_item_statuses
            .get(th)
            .map(clone_transaction_status)
            .ok_or_else(|| {
                QueryError::RPCError(RPCError::CallError(Status::not_found("not found")))
            })?;

        let summary = match txn_status {
            TransactionStatus::Received => {
                unimplemented!()
            }
            TransactionStatus::Committed(val) => {
                // Enable locally to sleep for 5 seconds to simulate the process until the tx is finalized.
                // Note: As this step would slow down testing in the CI pipeline it is disabled but can be used locally by removing the comment.
                // use std::thread;
                // use std::time::Duration;
                // thread::sleep(Duration::from_secs(5));

                // We only inserted one block item at the `GENESIS_BLOCK_HASH` in the test cases.
                val.get(&fixtures::chain::GENESIS_BLOCK_HASH.into())
                    .ok_or(QueryError::NotFound)?
                    .clone()
            }
            TransactionStatus::Finalized(val) => {
                // We only inserted one block item at the `GENESIS_BLOCK_HASH` in the test cases.
                val.get(&fixtures::chain::GENESIS_BLOCK_HASH.into())
                    .ok_or(QueryError::NotFound)?
                    .clone()
            }
        };

        Ok((GENESIS_BLOCK_HASH.into(), summary))
    }

    async fn get_account_credentials(
        &mut self,
        cred_id: CredentialRegistrationID,
        _bi: BlockIdentifier,
    ) -> QueryResult<AccountCredentials> {
        // CredentialRegistrationID does not implement Hash, hence we convert to bytes
        let cred_id_bytes = common::to_bytes(&cred_id);
        self.0
            .lock()
            .account_credentials
            .get(&cred_id_bytes)
            .cloned()
            .ok_or_else(|| {
                QueryError::RPCError(RPCError::CallError(Status::not_found("not found")))
            })
    }

    async fn get_identity_providers(
        &mut self,
        _bi: BlockIdentifier,
    ) -> QueryResult<Vec<IpInfo<IpPairing>>> {
        Ok(self.0.lock().ips.clone())
    }

    async fn get_anonymity_revokers(
        &mut self,
        _bi: BlockIdentifier,
    ) -> QueryResult<Vec<ArInfo<ArCurve>>> {
        Ok(self.0.lock().ars.clone())
    }

    fn box_clone(&self) -> Box<dyn NodeClient> {
        Box::new(self.clone())
    }
}

fn call_error(message: impl Into<String>) -> RPCError {
    RPCError::CallError(Status::internal(message))
}

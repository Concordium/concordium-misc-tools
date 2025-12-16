use crate::integration_test_helpers::fixtures;
use chrono::{DateTime, Utc};
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
use concordium_rust_sdk::base::transactions::{BlockItem, EncodedPayload};
use concordium_rust_sdk::common;
use concordium_rust_sdk::endpoints::{QueryResult, RPCResult};
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::types::{ArInfo, GlobalContext, IpInfo};
use concordium_rust_sdk::types::{CredentialRegistrationID, Nonce, TransactionStatus};
use concordium_rust_sdk::v2::{BlockIdentifier, QueryError, RPCError};
use credential_verification_service::node_client::{AccountCredentials, NodeClient};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use tonic::Status;

pub fn node_client(global_context: GlobalContext<ArCurve>) -> NodeClientStub {
    let inner = NodeClientStubInner {
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

    NodeClientStub(Arc::new(Mutex::new(inner)))
}

#[derive(Debug, Clone)]
pub struct NodeClientStub(Arc<Mutex<NodeClientStubInner>>);

impl NodeClientStub {
    pub fn stub_block_item_status(&self, txn_hash: TransactionHash, summary: TransactionStatus) {
        self.0.lock().block_item_statuses.insert(txn_hash, summary);
    }

    pub fn stub_account_credentials(
        &self,
        cred_id: CredentialRegistrationID,
        credentials: (AccountCredentials, AccountAddress),
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
pub struct NodeClientStubInner {
    global_context: GlobalContext<ArCurve>,
    ips: Vec<IpInfo<IpPairing>>,
    ars: Vec<ArInfo<ArCurve>>,
    account_sequence_number: Nonce,
    genesis_block_hash: BlockHash,
    block_slot_time: DateTime<Utc>,
    block_item_statuses: HashMap<TransactionHash, TransactionStatus>,
    account_credentials: HashMap<Vec<u8>, (AccountCredentials, AccountAddress)>,
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

#[async_trait::async_trait]
impl NodeClient for NodeClientStub {
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
        let txn_hash = fixtures::chain::generate_txn_hash();
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
        self.0
            .lock()
            .block_item_statuses
            .get(th)
            .map(clone_transaction_status)
            .ok_or_else(|| {
                QueryError::RPCError(call_error(format!(
                    "block item status not present in stub: {}",
                    th
                )))
            })
    }

    async fn get_account_credentials(
        &mut self,
        cred_id: CredentialRegistrationID,
        _bi: BlockIdentifier,
    ) -> QueryResult<(AccountCredentials, AccountAddress)> {
        // CredentialRegistrationID does not implement Hash, hence we convert to bytes
        let cred_id_bytes = common::to_bytes(&cred_id);
        self.0
            .lock()
            .account_credentials
            .get(&cred_id_bytes)
            .cloned()
            .ok_or_else(|| {
                QueryError::RPCError(call_error(format!(
                    "account credentials not present in stub: {}",
                    cred_id
                )))
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

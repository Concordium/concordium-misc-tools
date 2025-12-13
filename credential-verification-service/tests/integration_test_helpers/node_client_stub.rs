use crate::integration_test_helpers::fixtures;
use std::collections::HashMap;

use chrono::{DateTime, Utc};
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
use concordium_rust_sdk::base::transactions::{BlockItem, EncodedPayload};
use concordium_rust_sdk::base::web3id::v1::anchor::VerificationMaterialWithValidity;
use concordium_rust_sdk::endpoints::{QueryResult, RPCResult};
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::types::{ArInfo, GlobalContext, IpInfo};
use concordium_rust_sdk::types::{CredentialRegistrationID, Nonce, TransactionStatus};
use concordium_rust_sdk::v2::BlockIdentifier;
use concordium_rust_sdk::web3id::v1::VerifyError;
use credential_verification_service::node_client::NodeClient;
use parking_lot::Mutex;
use std::sync::Arc;

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
    send_block_items: HashMap<TransactionHash, BlockItem<EncodedPayload>>,
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
        self.0.lock().send_block_items.insert(txn_hash, bi.clone());
        Ok(txn_hash)
    }

    async fn get_cryptographic_parameters(
        &mut self,
        _bi: BlockIdentifier,
    ) -> QueryResult<GlobalContext<ArCurve>> {
        Ok(self.0.lock().global_context.clone())
    }

    async fn get_block_slot_time(&mut self, _bi: BlockIdentifier) -> QueryResult<DateTime<Utc>> {
        Ok(self.0.lock().block_slot_time.clone())
    }

    async fn get_block_item_status(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<TransactionStatus> {
        Ok(self
            .0
            .lock()
            .block_item_statuses
            .remove(&th)
            .expect("block_item_status"))
    }

    async fn get_account_credential_verification_material(
        &mut self,
        _cred_id: CredentialRegistrationID,
        _bi: BlockIdentifier,
    ) -> Result<VerificationMaterialWithValidity, VerifyError> {
        todo!()
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

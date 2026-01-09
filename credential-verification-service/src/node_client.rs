use chrono::{DateTime, Utc};
use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
use concordium_rust_sdk::base::transactions::{BlockItem, EncodedPayload};
use concordium_rust_sdk::common::Versioned;
use concordium_rust_sdk::common::types::{AccountAddress, CredentialIndex};
use concordium_rust_sdk::endpoints::{QueryResult, RPCError};
use concordium_rust_sdk::id::constants::{ArCurve, AttributeKind, IpPairing};
use concordium_rust_sdk::id::types::{
    AccountCredentialWithoutProofs, ArInfo, GlobalContext, IpInfo,
};
use concordium_rust_sdk::types::{
    BlockItemSummary, CredentialRegistrationID, Nonce, TransactionStatus,
};
use concordium_rust_sdk::v2;
use concordium_rust_sdk::v2::{AccountIdentifier, BlockIdentifier, QueryError, RPCResult, Upward};
use futures_util::TryStreamExt;
use prometheus_client::encoding::EncodeLabelSet;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::histogram;
use std::collections::BTreeMap;
use std::fmt::Debug;

/// Node interface used by the verifier service. Used to stub out node in tests
#[async_trait::async_trait]
pub trait NodeClient: Send + Sync + 'static + Debug {
    async fn wait_until_finalized(
        &mut self,
        hash: &TransactionHash,
    ) -> QueryResult<(BlockHash, BlockItemSummary)>;

    async fn get_next_account_sequence_number(
        &mut self,
        address: &AccountAddress,
    ) -> QueryResult<Nonce>;

    async fn get_genesis_block_hash(&mut self) -> QueryResult<BlockHash>;

    async fn send_block_item(
        &mut self,
        bi: &BlockItem<EncodedPayload>,
    ) -> RPCResult<TransactionHash>;

    async fn get_cryptographic_parameters(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<GlobalContext<ArCurve>>;

    async fn get_block_slot_time(&mut self, bi: BlockIdentifier) -> QueryResult<DateTime<Utc>>;

    async fn get_block_item_status(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<TransactionStatus>;

    async fn get_account_credentials(
        &mut self,
        cred_id: CredentialRegistrationID,
        bi: BlockIdentifier,
    ) -> QueryResult<AccountCredentials>;

    async fn get_identity_providers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<IpInfo<IpPairing>>>;

    async fn get_anonymity_revokers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<ArInfo<ArCurve>>>;

    fn boxed(self) -> Box<dyn NodeClient>
    where
        Self: Sized,
    {
        Box::new(self)
    }

    fn box_clone(&self) -> Box<dyn NodeClient>;
}

pub type AccountCredentials = BTreeMap<
    CredentialIndex,
    Versioned<Upward<AccountCredentialWithoutProofs<ArCurve, AttributeKind>>>,
>;

impl Clone for Box<dyn NodeClient> {
    fn clone(&self) -> Self {
        self.box_clone()
    }
}

/// Node interface using the node gRPC client
#[derive(Debug, Clone)]
pub struct NodeClientImpl {
    client: v2::Client,
}

impl NodeClientImpl {
    pub fn new(client: v2::Client) -> Self {
        Self { client }
    }
}

#[async_trait::async_trait]
impl NodeClient for NodeClientImpl {
    async fn wait_until_finalized(
        &mut self,
        hash: &TransactionHash,
    ) -> QueryResult<(BlockHash, BlockItemSummary)> {
        Ok(self.client.wait_until_finalized(hash).await?)
    }

    async fn get_next_account_sequence_number(
        &mut self,
        address: &AccountAddress,
    ) -> QueryResult<Nonce> {
        Ok(self
            .client
            .get_next_account_sequence_number(address)
            .await?
            .nonce)
    }

    async fn get_genesis_block_hash(&mut self) -> QueryResult<BlockHash> {
        Ok(self.client.get_consensus_info().await?.genesis_block)
    }

    async fn send_block_item(
        &mut self,
        bi: &BlockItem<EncodedPayload>,
    ) -> RPCResult<TransactionHash> {
        self.client.send_block_item(bi).await
    }

    async fn get_cryptographic_parameters(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<GlobalContext<ArCurve>> {
        self.client
            .get_cryptographic_parameters(bi)
            .await
            .map(|res| res.response)
    }

    async fn get_block_slot_time(&mut self, bi: BlockIdentifier) -> QueryResult<DateTime<Utc>> {
        Ok(self
            .client
            .get_block_info(bi)
            .await?
            .response
            .block_slot_time)
    }

    async fn get_block_item_status(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<TransactionStatus> {
        self.client.get_block_item_status(th).await
    }

    async fn get_account_credentials(
        &mut self,
        cred_id: CredentialRegistrationID,
        bi: BlockIdentifier,
    ) -> QueryResult<AccountCredentials> {
        let account_info = self
            .client
            .get_account_info(&AccountIdentifier::CredId(cred_id), bi)
            .await?;

        Ok(account_info.response.account_credentials)
    }

    async fn get_identity_providers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<IpInfo<IpPairing>>> {
        self.client
            .get_identity_providers(bi)
            .await?
            .response
            .map_err(|err| QueryError::RPCError(RPCError::CallError(err)))
            .try_collect()
            .await
    }

    async fn get_anonymity_revokers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<ArInfo<ArCurve>>> {
        self.client
            .get_anonymity_revokers(bi)
            .await?
            .response
            .map_err(|err| QueryError::RPCError(RPCError::CallError(err)))
            .try_collect()
            .await
    }

    fn box_clone(&self) -> Box<dyn NodeClient> {
        self.clone().boxed()
    }
}

#[derive(Debug, Clone, EncodeLabelSet, PartialEq, Eq, Hash)]
pub struct NodeRequestLabels {
    method: String,
    status: String,
}

/* Decorator for NodeClient that adds metrics collection
    inner is the original node client
    node_request_duration is the prometheus metric family for tracking request durations
    NodeRequestLabels are labels with with method name and status of the call with values "success" or "error"
*/
#[derive(Debug, Clone)]
pub struct NodeClientMetricsDecorator {
    inner: Box<dyn NodeClient>,
    node_request_duration: Family<NodeRequestLabels, histogram::Histogram>,
}

impl NodeClientMetricsDecorator {
    pub fn new(
        inner: Box<dyn NodeClient>,
        node_request_duration: Family<NodeRequestLabels, histogram::Histogram>,
    ) -> Self {
        Self {
            inner,
            node_request_duration,
        }
    }

    pub fn record_metrics<T, E>(
        &self,
        name: &str,
        result: &Result<T, E>,
        start_timer: tokio::time::Instant,
    ) {
        let status = if result.is_ok() { "success" } else { "error" };

        self.node_request_duration
            .get_or_create(&NodeRequestLabels {
                method: name.to_string(),
                status: status.to_string(),
            })
            .observe(start_timer.elapsed().as_secs_f64());
    }
}

#[async_trait::async_trait]
impl NodeClient for NodeClientMetricsDecorator {
    async fn wait_until_finalized(
        &mut self,
        hash: &TransactionHash,
    ) -> QueryResult<(BlockHash, BlockItemSummary)> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.wait_until_finalized(hash).await;

        self.record_metrics("wait_until_finalized", &result, start_timer);

        result
    }

    async fn get_next_account_sequence_number(
        &mut self,
        address: &AccountAddress,
    ) -> QueryResult<Nonce> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_next_account_sequence_number(address).await;

        self.record_metrics("get_next_account_sequence_number", &result, start_timer);

        result
    }

    async fn get_genesis_block_hash(&mut self) -> QueryResult<BlockHash> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_genesis_block_hash().await;

        self.record_metrics("get_genesis_block_hash", &result, start_timer);

        result
    }

    async fn send_block_item(
        &mut self,
        bi: &BlockItem<EncodedPayload>,
    ) -> RPCResult<TransactionHash> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.send_block_item(bi).await;

        self.record_metrics("send_block_item", &result, start_timer);

        result
    }

    async fn get_cryptographic_parameters(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<GlobalContext<ArCurve>> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_cryptographic_parameters(bi).await;

        self.record_metrics("get_cryptographic_parameters", &result, start_timer);

        result
    }

    async fn get_block_slot_time(&mut self, bi: BlockIdentifier) -> QueryResult<DateTime<Utc>> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_block_slot_time(bi).await;

        self.record_metrics("get_block_slot_time", &result, start_timer);

        result
    }

    async fn get_block_item_status(
        &mut self,
        th: &TransactionHash,
    ) -> QueryResult<TransactionStatus> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_block_item_status(th).await;

        self.record_metrics("get_block_item_status", &result, start_timer);

        result
    }

    async fn get_account_credentials(
        &mut self,
        cred_id: CredentialRegistrationID,
        bi: BlockIdentifier,
    ) -> QueryResult<AccountCredentials> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_account_credentials(cred_id, bi).await;

        self.record_metrics("get_account_credentials", &result, start_timer);

        result
    }

    async fn get_identity_providers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<IpInfo<IpPairing>>> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_identity_providers(bi).await;

        self.record_metrics("get_identity_providers", &result, start_timer);

        result
    }

    async fn get_anonymity_revokers(
        &mut self,
        bi: BlockIdentifier,
    ) -> QueryResult<Vec<ArInfo<ArCurve>>> {
        let start_timer = tokio::time::Instant::now();

        let result = self.inner.get_anonymity_revokers(bi).await;

        self.record_metrics("get_anonymity_revokers", &result, start_timer);

        result
    }

    fn box_clone(&self) -> Box<dyn NodeClient> {
        Box::new(NodeClientMetricsDecorator {
            inner: self.inner.box_clone(),
            node_request_duration: self.node_request_duration.clone(),
        })
    }
}

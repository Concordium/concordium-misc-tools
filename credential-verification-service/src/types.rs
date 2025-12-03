use axum::{Json, http::StatusCode};
use concordium_rust_sdk::{
    base::web3id::v1::anchor::{self, RequestedSubjectClaims},
    common::cbor,
    types::{Nonce, WalletAccount},
    v2,
    web3id::{did::Network, v1::CreateAnchorError},
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

/// Holds the service state in memory.
///
/// Note: A new instance of this struct is created whenever the service restarts.
pub struct Service {
    /// The client to interact with the node.
    pub node_client: v2::Client,
    /// The network of the connected node.  
    pub network: Network,
    /// The key and address of the account submitting the anchor transactions on-chain.
    pub account_keys: Arc<WalletAccount>,
    /// The current nonce of the account submitting the anchor transactions on-chain.
    pub nonce: Arc<Mutex<Nonce>>,
    /// The number of seconds in the future when the anchor transactions should expiry.  
    pub transaction_expiry_secs: u32,
}

/// Parameters posted to this service when calling the API
/// endpoint `/verifiable-presentations/create-verification-request`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct VerificationRequestParams {
    /// The nonce included in the verification request context.  
    /// This nonce must be freshly and randomly generated for each request so that the
    /// verification request cannot be inferred from the on-chain request anchor hash
    /// by attempting to guess its preimage.  
    /// In short: to keep the anchor hash random, this nonce must be truly random.
    pub nonce: anchor::Nonce,
    /// An identifier for some connection (e.g. wallet-connect topic) included in the verification request context.
    pub connection_id: String,
    /// A general purpose string value included in the verification request context.
    pub context_string: String,
    /// The subject claims being requested to be proven.
    pub requested_claims: Vec<RequestedSubjectClaims>,
    /// Additional public info which will be included in the anchor transaction (VRA)
    /// that is submitted on-chain.
    pub public_info: HashMap<String, cbor::value::Value>,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Unable to submit anchor transaction on chain successfully: {0}.")]
    SubmitAnchorTransaction(#[from] CreateAnchorError),
    #[error("Unable to submit transaction on chain successfully due to nonce mismatch: {0}.")]
    NonceMismatch(CreateAnchorError),
}

impl axum::response::IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let r = match self {
            ServerError::SubmitAnchorTransaction(error) => {
                tracing::error!("Internal error: {error}.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal error.".to_string()),
                )
            }
            ServerError::NonceMismatch(error) => {
                tracing::error!("Service unavailable: {error}.");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json("Service unavailable.".to_string()),
                )
            }
        };
        r.into_response()
    }
}

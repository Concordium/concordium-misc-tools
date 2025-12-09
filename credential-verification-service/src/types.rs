use axum::{Json, http::StatusCode};
use concordium_rust_sdk::{
    types::{Nonce, WalletAccount},
    v2,
    web3id::{
        did::Network,
        v1::{CreateAnchorError, VerifyError},
    },
};
use std::sync::Arc;
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

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Unable to submit anchor transaction on chain successfully: {0}.")]
    SubmitAnchorTransaction(#[from] CreateAnchorError),
    #[error("Unable to submit anchor transaction on chain: {0}.")]
    PresentationVerifificationFailed(#[from] VerifyError),
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
            ServerError::PresentationVerifificationFailed(error) => {
                let error_message = format!("Presentation Verification Failed: {}", error);
                tracing::error!(error_message);
                (StatusCode::BAD_REQUEST, Json(error_message))
            }
        };
        r.into_response()
    }
}

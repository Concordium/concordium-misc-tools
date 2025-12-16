use crate::node_client::NodeClient;
use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use concordium_rust_sdk::base::transactions::TooLargeError;
use concordium_rust_sdk::common::cbor::CborSerializationError;
use concordium_rust_sdk::id::types::IpIdentity;
use concordium_rust_sdk::types::CredentialRegistrationID;
use concordium_rust_sdk::types::hashes::TransactionHash;
use concordium_rust_sdk::web3id::did::Network;

use crate::txn_submitter::TransactionSubmitter;

/// Holds the service state in memory.
///
/// Note: A new instance of this struct is created whenever the service restarts.
#[derive(Debug, Clone)]
pub struct Service {
    /// The client to interact with the node.
    pub node_client: Box<dyn NodeClient>,
    /// The network of the connected node.  
    pub network: Network,
    /// Submitter for transactions
    pub transaction_submitter: TransactionSubmitter,
}

/// Extractor with build in error handling. Like [axum::Json](Json) but will use [`RejectionError`] for rejection errors
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(RejectionError))]
#[allow(dead_code)]
pub struct AppJson<T>(pub T);

/// Error returned by REST endpoint handlers. Will
/// be mapped to the right HTTP response (HTTP code and custom
/// error body) by the axum middleware
#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("{0:#}")]
    Anyhow(#[from] anyhow::Error),
    #[error("request anchor transaction {0} not found")]
    RequestAnchorTransactionNotFound(TransactionHash),
    #[error("request anchor transaction {0} not finalized")]
    RequestAnchorTransactionNotFinalized(TransactionHash),
    #[error("request anchor transaction {0} not a register data transaction")]
    RequestAnchorTransactionNotRegisterData(TransactionHash),
    #[error("error decoding registered data in request anchor transaction {0}: {1}")]
    RequestAnchorDecode(TransactionHash, CborSerializationError),
    #[error("identity provider {0} not found")]
    IdentityProviderNotFound(IpIdentity),
    #[error("account credential {0} not found")]
    AccountCredentialNotFound(Box<CredentialRegistrationID>),
    #[error("anchor public info too big: {0}")]
    AnchorPublicInfoTooBig(TooLargeError),
}

/// Error for handling rejections of invalid requests.
/// Will be mapped to the right HTTP response (HTTP code and custom
/// error body) by the axum middleware.
///
/// See <https://docs.rs/axum/latest/axum/extract/index.html#customizing-extractor-responses>
#[derive(Debug, thiserror::Error)]
pub enum RejectionError {
    #[error("invalid json in request: {0}")]
    JsonRejection(#[from] JsonRejection),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        match self {
            ServerError::Anyhow(_) => {
                tracing::error!("internal error: {self}");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal server error").into_response()
            }
            ServerError::RequestAnchorTransactionNotRegisterData(_)
            | ServerError::RequestAnchorTransactionNotFound(_)
            | ServerError::RequestAnchorTransactionNotFinalized(_)
            | ServerError::RequestAnchorDecode(_, _)
            | ServerError::IdentityProviderNotFound(_)
            | ServerError::AccountCredentialNotFound(_)
            | ServerError::AnchorPublicInfoTooBig(_) => {
                (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()).into_response()
            }
        }
    }
}

impl IntoResponse for RejectionError {
    fn into_response(self) -> Response {
        let status = match self {
            RejectionError::JsonRejection(_) => StatusCode::BAD_REQUEST,
        };
        (status, self.to_string()).into_response()
    }
}

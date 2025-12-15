use crate::node_client::NodeClient;
use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use concordium_rust_sdk::{
    types::{Nonce, WalletAccount},
    web3id::did::Network,
};
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::Mutex;
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
    pub txn_submitter: TransactionSubmitter
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
}

/// Error for handling rejections of invalid requests.
/// Will be mapped to the right HTTP response (HTTP code and custom
/// error body) by the axum middleware.
///
/// See <https://docs.rs/axum/latest/axum/extract/index.html#customizing-extractor-responses>
#[derive(Debug, thiserror::Error)]
pub enum RejectionError {
    #[error("invalid json in request")]
    JsonRejection(#[from] JsonRejection),
}

fn error_response(err: &impl Display, http_status: StatusCode) -> Response {
    (http_status, err.to_string()).into_response()
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = match &self {
            err @ ServerError::Anyhow(_) => {
                tracing::warn!("internal error: {err}");

                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        error_response(&self, status)
    }
}

impl IntoResponse for RejectionError {
    fn into_response(self) -> Response {
        let status = match self {
            RejectionError::JsonRejection(_) => StatusCode::BAD_REQUEST,
        };
        error_response(&self, status)
    }
}

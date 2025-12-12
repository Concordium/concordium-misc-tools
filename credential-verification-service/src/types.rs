use axum::extract::FromRequest;
use axum::extract::rejection::JsonRejection;
use axum::response::{IntoResponse, Response};
use axum::{Json, http::StatusCode};
use concordium_rust_sdk::{
    types::{Nonce, WalletAccount},
    v2::{self, QueryError},
    web3id::{
        did::Network,
        v1::{CreateAnchorError, VerifyError},
    },
};
use std::fmt::Display;
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

/// Extractor with build in error handling. Like [axum::Json](Json) but will use [`RejectionError`] for rejection errors
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(RejectionError))]
#[allow(dead_code)]
struct AppJson<T>(T);

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
enum RejectionError {
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

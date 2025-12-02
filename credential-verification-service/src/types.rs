use axum::{Json, http::StatusCode};
use concordium_rust_sdk::web3id::v1::CreateAnchorError;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Unable to submit anchor transaction on chain successfully: {0}.")]
    SubmitAnchorTransaction(#[from] CreateAnchorError),
    #[error("Unable to submit transaction on chain successfully: {0}.")]
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

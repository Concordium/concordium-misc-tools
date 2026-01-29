use crate::api_types::{ErrorBody, ErrorDetail, ErrorResponse};
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
use std::time::Duration;

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
    /// Timeout for waiting for the anchor transaction to finalize when verifying a proof.
    pub anchor_wait_for_finalization_timeout: Duration,
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
    #[error("Payload validation failed: {0}")]
    PayloadValidation(#[from] ValidationError),
    #[error("request anchor transaction {0} not found")]
    RequestAnchorTransactionNotFound(TransactionHash),
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
    #[error("Timeout happened when waiting for request anchor transaction {0} to finalize")]
    TimeoutWaitingForFinalization(TransactionHash),
}

/// Error for validating the statements/claims in a request to this service.
#[derive(Debug, thiserror::Error)]
#[error("validation failed")]
pub struct ValidationError {
    pub details: Vec<ErrorDetail>,
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

impl IntoResponse for RejectionError {
    fn into_response(self) -> Response {
        // TODO - this should be replaced in future with trace id that is
        // generated from some utility, or logic that parses it from a
        // request header for example.
        let trace_id = "dummy".to_string();

        tracing::error!("Invalid json in the request: {self}");

        let json_message = self.to_string();

        let body = ErrorResponse {
            error: ErrorBody {
                code: "INVALID_JSON".to_string(),
                message: json_message,
                trace_id,
                retryable: false,
                details: vec![],
            },
        };

        (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        // TODO - this should be replaced in future with trace id that is
        // generated from some utility, or logic that parses it from a
        // request header for example.
        let trace_id = "dummy".to_string();

        match self {
            ServerError::Anyhow(_) => {
                tracing::error!("internal error: {self}");

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "INTERNAL_ERROR".to_string(),
                        message: "An error has occurred while processing the request. Please try again later".to_string(),
                        trace_id,
                        retryable: true,
                        details: vec![],
                    },
                };
                (StatusCode::INTERNAL_SERVER_ERROR, axum::Json(body)).into_response()
            }

            ServerError::PayloadValidation(validation_error) => {
                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "VALIDATION_ERROR".to_string(),
                        message: "Validation errors have occurred. Please check the details below for more information.".to_string(),
                        trace_id,
                        retryable: false,
                        details: validation_error.details,
                    }
                };

                (StatusCode::BAD_REQUEST, axum::Json(body)).into_response()
            }

            ServerError::RequestAnchorTransactionNotRegisterData(hash) => {
                tracing::warn!("request anchor transaction not registered. Error: {self}");

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "REQUEST_ANCHOR_NOT_REGISTERED".to_string(),
                        message: format!("request anchor transaction {hash} not found"),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::RequestAnchorTransactionNotFound(hash) => {
                tracing::warn!("request anchor transaction not found. Error: {self}");

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "REQUEST_ANCHOR_NOT_FOUND".to_string(),
                        message: format!("request anchor transaction {hash} not found"),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::RequestAnchorDecode(hash, error) => {
                tracing::warn!(
                    "request anchor decode issue for transaction hash: {hash}. Error: {error}"
                );

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "REQUEST_ANCHOR_DECODE_ISSUE".to_string(),
                        message: format!(
                            "request anchor transaction {hash} encountered a decoding issue."
                        ),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::IdentityProviderNotFound(ip_identity) => {
                tracing::warn!("identity provider could not be found. {self}");

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "IDENTITY_PROVIDER_NOT_FOUND".to_string(),
                        message: format!("Identity provider could not be found {ip_identity}"),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::AccountCredentialNotFound(credential_registration_id) => {
                tracing::warn!(
                    "Account credential could not be found: {credential_registration_id}."
                );

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "ACCOUNT_CREDENTIAL_NOT_FOUND".to_string(),
                        message: format!(
                            "Account credential could not be found: {credential_registration_id}"
                        ),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::TimeoutWaitingForFinalization(hash) => {
                tracing::warn!(
                    "Timeout waiting for finalization for transaction hash: {hash}. Error: {self}"
                );

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "TIMEOUT_WAITING_FOR_FINALIZATION".to_string(),
                        message: format!(
                            "Timeout waiting for transaction hash: {hash} to finalize."
                        ),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }

            ServerError::AnchorPublicInfoTooBig(error) => {
                tracing::warn!("Anchor public info provided was too big: {error}.");

                let body = ErrorResponse {
                    error: ErrorBody {
                        code: "ANCHOR_PUBLIC_INFO_TOO_BIG".to_string(),
                        message: format!(
                            "provided anchor public info provided was too big. {self}"
                        ),
                        trace_id,
                        retryable: false,
                        details: vec![],
                    },
                };
                (StatusCode::UNPROCESSABLE_ENTITY, axum::Json(body)).into_response()
            }
        }
    }
}

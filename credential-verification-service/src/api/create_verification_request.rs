//! Handler for create-verification-request endpoint.

use crate::api::util;
use crate::api_types::{ErrorBody, ErrorResponse};
//use crate::api::validate_payload::payload_validation;
use crate::types::AppJson;
use crate::validation::create_verification_api_request_validator;
use crate::{api_types::CreateVerificationRequest, types::Service};
use axum::{Json, extract::State};
use concordium_rust_sdk::base::web3id::v1::anchor::{ContextLabel, Nonce};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    LabeledContextProperty, UnfilledContextInformationBuilder, VerificationRequest,
    VerificationRequestDataBuilder,
};
use std::sync::Arc;
use tracing::debug;

pub async fn create_verification_request(
    State(state): State<Arc<Service>>,
    AppJson(params): AppJson<CreateVerificationRequest>,
) -> Result<Json<VerificationRequest>, ErrorResponse> {
    // Validate format of statements/claims in the payload request.
    //payload_validation(params.requested_claims.clone())?;
    create_verification_api_request_validator::validate(&params)?;

    let context_nonce = Nonce(rand::random());
    let context_builder = UnfilledContextInformationBuilder::new()
        .given(LabeledContextProperty::Nonce(context_nonce))
        .given(LabeledContextProperty::ConnectionId(params.connection_id))
        .given(LabeledContextProperty::ResourceId(params.resource_id))
        .requested(ContextLabel::BlockHash);

    let context_builder = if let Some(context_string) = params.context_string {
        context_builder.given(LabeledContextProperty::ContextString(context_string))
    } else {
        context_builder
    };

    let context = context_builder.build();

    let mut builder = VerificationRequestDataBuilder::new(context);
    for claim in params.requested_claims {
        builder = builder.subject_claim(claim);
    }
    let verification_request_data = builder.build();

    // Create the request anchor
    let verification_request_anchor = verification_request_data.to_anchor(params.public_info);
    let anchor_data = util::anchor_to_registered_data(&verification_request_anchor)
        .map_err(|e| {
                debug!("A server error has occurred while converting the verification request to anchor. Verification request: {:?}. Error: {:?}", &verification_request_data, e);
                ErrorResponse {
                    error: ErrorBody {
                        code: "INTERNAL_ERROR".to_string(),
                        message: "An Error has occurred processing this request. Please try again later.".to_string(),
                        retryable: true,
                        trace_id: "DUMMY TODO".to_string(),
                        details: vec![]
                    }
                }
            })?;

    // Submit the anchor
    let anchor_transaction_hash = state
        .transaction_submitter
        .submit_register_data_txn(anchor_data)
        .await
        .map_err(|e| {
            debug!("A server error has occurred while submitting the register data transaction. Error: {:?}", e);
            ErrorResponse {
                error: ErrorBody {
                    code: "INTERNAL_ERROR".to_string(),
                    message: "An Error has occurred processing this request. Please try again later.".to_string(),
                    retryable: true,
                    trace_id: "DUMMY TODO".to_string(),
                    details: vec![]
                }
            }
        })?;

    let verification_request = VerificationRequest {
        context: verification_request_data.context,
        subject_claims: verification_request_data.subject_claims,
        anchor_transaction_hash,
    };

    Ok(Json(verification_request))
}

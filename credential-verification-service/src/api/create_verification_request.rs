//! Handler for create-verification-request endpoint.

use crate::types::AppJson;
use crate::{
    api_types::CreateVerificationRequest,
    types::{ServerError, Service},
};
use anyhow::Context;
use axum::{extract::State, Json};
use concordium_rust_sdk::base::transactions::ExactSizeTransactionSigner;
use concordium_rust_sdk::base::web3id::v1::anchor::{ContextLabel, Nonce, VerificationRequestData};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::types::RegisteredData;
use concordium_rust_sdk::web3id::v1::CreateAnchorError;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    LabeledContextProperty, UnfilledContextInformationBuilder, VerificationRequest,
    VerificationRequestDataBuilder,
};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn create_verification_request(
    State(state): State<Arc<Service>>,
    AppJson(params): AppJson<CreateVerificationRequest>,
) -> Result<Json<VerificationRequest>, ServerError> {
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

    let anchor_register_data = create_request_anchor_register_data(
        verification_request_data.clone(),
        params.public_info.clone(),
    )
    .context("create request anchor registration data")?;

    let anchor_transaction_hash = state
        .transaction_submitter
        .submit_register_data_txn(anchor_register_data)
        .await
        .context("submit register data transaction")?;

    let verification_request = VerificationRequest {
        context: verification_request_data.context,
        subject_claims: verification_request_data.subject_claims,
        anchor_transaction_hash,
    };

    Ok(Json(verification_request))
}

/// Create data to be registered in the request anchor
fn create_request_anchor_register_data<S: ExactSizeTransactionSigner>(
    verification_request_data: VerificationRequestData,
    public_info: Option<HashMap<String, cbor::value::Value>>,
) -> Result<RegisteredData, CreateAnchorError> {
    let verification_request_anchor = verification_request_data.to_anchor(public_info);
    let cbor = cbor::cbor_encode(&verification_request_anchor)?;
    let register_data = RegisteredData::try_from(cbor)?;
    Ok(register_data)
}

//! Handler for the verification endpoints.
use crate::{
    api_types::VerifyPresentationRequest,
    types::{ServerError, Service},
};
use axum::{Json, extract::State};
use concordium_rust_sdk::{
    common::{cbor, types::TransactionTime},
    v2::{BlockIdentifier},
    web3id::{self, did::Network, v1::{
        AnchorTransactionMetadata, AuditRecordArgument, PresentationVerificationData
    }},
};
use std::{collections::HashMap, sync::Arc};

/// Verify Presentation endpoint handler.
/// Accepts a VerifyPresentationRequest payload and calls the Rust SDK function `verify_presentation_and_submit_audit_anchor` to perform the cryptographic verification, context checking and Verifiable request anchor checks, and finally submits the Verifiable Audit Anchor on chain
pub async fn verify_presentation(
    state: State<Arc<Service>>,
    Json(verify_presentation_request): Json<VerifyPresentationRequest>,
) -> Result<Json<PresentationVerificationData>, ServerError> {
    // Transaction should expiry after some seconds.
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

    // client
    let mut client = state.node_client.clone();

    // TODO - network should be taken from env config later
    let network = Network::Testnet;

    // Get the current nonce for the backend wallet and lock it. This is necessary
    // since it is possible that API requests come in parallel. The nonce is
    // increased by 1 and its lock is released after the transaction is submitted to
    // the blockchain.
    let account_sequence_number = state.nonce.lock().await;

    let anchor_transaction_metadata = AnchorTransactionMetadata {
        signer: &state.account_keys,
        sender: state.account_keys.address,
        account_sequence_number: *account_sequence_number,
        expiry,
    };

    // TODO - Audit record Argument creation - public info needs to be taken in here probably throught the VerifyPresentationRequest, audit record id could be the original nonce that was associated with the verification request
    let public_info: Option<HashMap<String, cbor::value::Value>> = Some(HashMap::new());
    let audit_record_argument = AuditRecordArgument {
        audit_record_id: "dummy_for_now".to_string(),
        public_info: public_info,
        audit_record_anchor_transaction_metadata: anchor_transaction_metadata,
    };

    let presentation_verification_data_result = web3id::v1::verify_presentation_and_submit_audit_anchor(
        &mut client,
        network,
        BlockIdentifier::LastFinal,
        verify_presentation_request.verification_request,
        verify_presentation_request.presentation,
        audit_record_argument,
    )
    .await;

    match presentation_verification_data_result {
        Ok(result) => Ok(Json(result)),
        Err(e) => Err(ServerError::PresentationVerifificationFailed(e))
    }
}

//! Handler for the verification endpoints.
use crate::{
    api_types::{VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse},
    types::{ServerError, Service},
};
use axum::{Json, extract::State};
use concordium_rust_sdk::{
    base::web3id::v1::anchor::PresentationVerificationResult,
    common::{cbor, types::TransactionTime},
    v2::BlockIdentifier,
    web3id::{
        self,
        v1::{AnchorTransactionMetadata, AuditRecordArgument},
    },
};
use std::{collections::HashMap, sync::Arc};

/// Verify Presentation endpoint handler.
/// Accepts a VerifyPresentationRequest payload and calls the Rust SDK function `verify_presentation_and_submit_audit_anchor` to perform the cryptographic verification, context checking and Verifiable request anchor checks, and finally submits the Verifiable Audit Anchor on chain
pub async fn verify_presentation(
    state: State<Arc<Service>>,
    Json(verify_presentation_request): Json<VerifyPresentationRequest>,
) -> Result<Json<VerifyPresentationResponse>, ServerError> {
    // Transaction should expiry after some seconds.
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

    // client
    let mut client = state.node_client.clone();

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

    // TODO - fix public info later after merge of the other PR
    let public_info: Option<HashMap<String, cbor::value::Value>> = Some(HashMap::new());
    let audit_record_argument = AuditRecordArgument {
        audit_record_id: verify_presentation_request.audit_record_id,
        public_info: public_info,
        audit_record_anchor_transaction_metadata: anchor_transaction_metadata,
    };

    let presentation_verification_data_result =
        web3id::v1::verify_presentation_and_submit_audit_anchor(
            &mut client,
            state.network,
            BlockIdentifier::LastFinal,
            verify_presentation_request.verification_request,
            verify_presentation_request.presentation,
            audit_record_argument,
        )
        .await;

    match presentation_verification_data_result {
        Ok(presentation_verification_data) => {
            let result = match presentation_verification_data.verification_result {
                PresentationVerificationResult::Verified => VerificationResult::Verified,
                PresentationVerificationResult::Failed(e) => {
                    VerificationResult::Failed(e.to_string())
                }
            };

            let verify_presentation_response = VerifyPresentationResponse {
                result,
                anchor_transaction_hash: presentation_verification_data.anchor_transaction_hash,
                verification_audit_record: presentation_verification_data.audit_record,
            };
            Ok(Json(verify_presentation_response))
        }
        Err(e) => Err(ServerError::PresentationVerifificationFailed(e)),
    }
}

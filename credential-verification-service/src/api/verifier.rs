//! Handler for the verification endpoints.
use crate::{
    api_types::{VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse},
    types::{ServerError, Service},
};
use axum::{Json, extract::State};
use concordium_rust_sdk::{
    base::web3id::v1::anchor::PresentationVerificationResult,
    common::{cbor, types::TransactionTime},
    types::WalletAccount,
    v2::{BlockIdentifier, QueryError, RPCError},
    web3id::{
        self,
        v1::{AnchorTransactionMetadata, AuditRecordArgument, VerifyError},
    },
};
use std::{collections::HashMap, sync::Arc};

/// Verify Presentation endpoint handler.
/// Accepts a VerifyPresentationRequest payload and calls the Rust SDK function `verify_presentation_and_submit_audit_anchor` to perform the cryptographic verification, context checking and Verifiable request anchor checks, and finally submits the Verifiable Audit Anchor on chain
pub async fn verify_presentation(
    state: State<Arc<Service>>,
    Json(verify_presentation_request): Json<VerifyPresentationRequest>,
) -> Result<Json<VerifyPresentationResponse>, ServerError> {
    // client
    let mut client = state.node_client.clone();

    // lock for the account sequence nonce, and build the audit record argument
    let mut account_sequence_number = state.nonce.lock().await;
    let audit_record_argument = build_audit_record(
        &state,
        &verify_presentation_request,
        *account_sequence_number,
    )
    .await;

    // verify the presentation and submit the audit anchor.
    // Clone the verify presentation request, as the retry scenario also needs it.
    let verify_presentation_request_clone = verify_presentation_request.clone();
    let presentation_verification_data_result =
        web3id::v1::verify_presentation_and_submit_audit_anchor(
            &mut client,
            state.network,
            BlockIdentifier::LastFinal,
            verify_presentation_request_clone.verification_request,
            verify_presentation_request_clone.presentation,
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

            // Update the account sequence number now as we successfully verfied and submitted the audit anchor
            *account_sequence_number = account_sequence_number.next();
            Ok(Json(verify_presentation_response))
        }
        Err(e) => {
            if let VerifyError::Query(QueryError::RPCError(RPCError::CallError(ref err))) = e {
                let msg = err.message();
                let is_nonce_err = msg == "Duplicate nonce" || msg == "Nonce too large";

                if is_nonce_err {
                    tracing::warn!(
                        "Unable to submit transaction on-chain successfully due to account nonce mismatch: {}.
                        Account nonce will be re-freshed and transaction will be re-submitted.",
                        msg
                    );

                    // Refresh nonce
                    let nonce_response = client
                        .get_next_account_sequence_number(&state.account_keys.address)
                        .await
                        .map_err(|e| ServerError::SubmitAnchorTransaction(e.into()))?;

                    *account_sequence_number = nonce_response.nonce;

                    tracing::info!("Refreshed account nonce successfully.");

                    // Retry anchor transaction.
                    let retry_audit_record_argument = build_audit_record(
                        &state,
                        &verify_presentation_request,
                        *account_sequence_number,
                    )
                    .await;

                    // try to verify the presentation with a new audit record argument with the updated nonce sequence number
                    let verify_presentation_request = verify_presentation_request.clone();
                    let presentation_verification_data =
                        web3id::v1::verify_presentation_and_submit_audit_anchor(
                            &mut client,
                            state.network,
                            BlockIdentifier::LastFinal,
                            verify_presentation_request.verification_request,
                            verify_presentation_request.presentation,
                            retry_audit_record_argument,
                        )
                        .await?;

                    tracing::info!(
                        "Successfully submitted anchor transaction after the account nonce was refreshed."
                    );

                    *account_sequence_number = account_sequence_number.next();

                    let result = match presentation_verification_data.verification_result {
                        PresentationVerificationResult::Verified => VerificationResult::Verified,
                        PresentationVerificationResult::Failed(e) => {
                            VerificationResult::Failed(e.to_string())
                        }
                    };

                    let verify_presentation_response = VerifyPresentationResponse {
                        result,
                        anchor_transaction_hash: presentation_verification_data
                            .anchor_transaction_hash,
                        verification_audit_record: presentation_verification_data.audit_record,
                    };

                    // Update the account sequence number now as we successfully verfied and submitted the audit anchor
                    *account_sequence_number = account_sequence_number.next();
                    return Ok(Json(verify_presentation_response));
                }
            }

            Err(ServerError::PresentationVerifificationFailed(e))
        }
    }
}

/// Helper function to build the Audit record Argument that will be used in the verify presentation call
async fn build_audit_record<'s>(
    state: &'s State<Arc<Service>>,
    verify_presentation_request: &VerifyPresentationRequest,
    account_sequence_number: concordium_rust_sdk::types::Nonce,
) -> AuditRecordArgument<&'s Arc<WalletAccount>> {
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

    let audit_record_anchor_transaction_metadata = AnchorTransactionMetadata {
        signer: &state.account_keys,
        sender: state.account_keys.address,
        account_sequence_number,
        expiry,
    };

    // TODO - fix public info later after merge of the other PR
    let public_info: Option<HashMap<String, cbor::value::Value>> = Some(HashMap::new());
    AuditRecordArgument {
        audit_record_id: verify_presentation_request.audit_record_id.clone(),
        public_info,
        audit_record_anchor_transaction_metadata,
    }
}

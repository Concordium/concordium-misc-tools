//! Handler for the verification endpoints.
use crate::api::util::QueryErrorExt;
use crate::{
    api_types::{VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse},
    types::{ServerError, Service},
};
use anyhow::Context;
use axum::{Json, extract::State};
use concordium_rust_sdk::web3id::v1::CreateAnchorError;
use concordium_rust_sdk::{
    base::web3id::v1::anchor::{
        self, PresentationVerificationResult, VerificationAuditRecord, VerificationContext,
    },
    common::types::TransactionTime,
    types::WalletAccount,
    v2::{self, BlockIdentifier, QueryError, RPCError},
    web3id::v1::{
        self, AnchorTransactionMetadata, AuditRecordArgument, PresentationVerificationData,
        VerifyError,
    },
};
use std::sync::Arc;
use crate::types::AppJson;

/// Verify Presentation endpoint handler.
/// Accepts a VerifyPresentationRequest payload and calls the Rust SDK function `verify_presentation_with_request_anchor`
/// to perform the cryptographic verification, context checking and Verifiable request anchor checks, and calls
/// `submit_verification_audit_record_anchor` to publish the audit anchor on chain, only when the verification has succeeded.
pub async fn verify_presentation(
    state: State<Arc<Service>>,
    AppJson(verify_presentation_request): AppJson<VerifyPresentationRequest>,
) -> Result<Json<VerifyPresentationResponse>, ServerError> {
    let block_identifier = BlockIdentifier::LastFinal;

    let mut client = state.node_client.clone();

    // Verify the presentation with respect to the verification request anchor.
    // note: we do not lock for the account nonce until the anchor submission
    let presentation_verification_result = verify_presentation_with_request_anchor(
        &mut client,
        block_identifier,
        &verify_presentation_request,
        &state,
    )
    .await?;

    // Lock for account nonce now before we call to create and submit the audit anchor
    let mut account_sequence_number = state.nonce.lock().await;

    // create and submit the audit anchor on chain
    let presentation_verification_data_result = create_and_submit_audit_anchor(
        &mut client,
        &verify_presentation_request,
        &state,
        presentation_verification_result,
        *account_sequence_number,
    )
    .await;

    // Check now if the presentation verification was ok
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

            // increment states nonce now
            *account_sequence_number = account_sequence_number.next();
            Ok(Json(verify_presentation_response))
        }
        Err(CreateAnchorError::Query(err)) if err.is_account_sequence_number_error() => {
            tracing::warn!(
                "Unable to submit transaction on-chain successfully due to account nonce mismatch. Account nonce will be refreshed and transaction will be re-submitted: {}",
                err
            );

            let mut client = state.node_client.clone();

            // Refresh nonce
            let nonce_response = client
                .get_next_account_sequence_number(&state.account_keys.address)
                .await
                .context("get next account sequence number")?;

            // resubmit the audit anchor with the updated account sequence number
            *account_sequence_number = nonce_response.nonce;
            tracing::info!("Refreshed account nonce successfully.");
            let presentation_verification_data_result = create_and_submit_audit_anchor(
                &mut client,
                &verify_presentation_request,
                &state,
                presentation_verification_result,
                *account_sequence_number,
            )
            .await
            .context("submit audit anchor")?;

            let result = match presentation_verification_data_result.verification_result {
                PresentationVerificationResult::Verified => VerificationResult::Verified,
                PresentationVerificationResult::Failed(e) => {
                    VerificationResult::Failed(e.to_string())
                }
            };

            let verify_presentation_response = VerifyPresentationResponse {
                result,
                anchor_transaction_hash: presentation_verification_data_result
                    .anchor_transaction_hash,
                verification_audit_record: presentation_verification_data_result.audit_record,
            };

            // finally increase the nonce in the state
            *account_sequence_number = account_sequence_number.next();

            Ok(Json(verify_presentation_response))
        }
        Err(err) => Err(anyhow::Error::from(err)
            .context("create and submit request anchor")
            .into()),
    }
}

/// Perform the full verification of the presentation with respect to the Verification Request Anchor.
/// This allows us to break out the verification and audit anchor submission into separate functionality
/// so that we only lock the account nonce for the anchor transaction submission, and not during the verification
/// process that this function follows.
async fn verify_presentation_with_request_anchor(
    client: &mut v2::Client,
    block_identifier: BlockIdentifier,
    verify_presentation_request: &VerifyPresentationRequest,
    state: &State<Arc<Service>>,
) -> Result<PresentationVerificationResult, ServerError> {
    let global_context = client
        .get_cryptographic_parameters(block_identifier)
        .await
        .context("get cryptographic parameters")?
        .response;

    let block_info = client
        .get_block_info(block_identifier)
        .await
        .context("get block info")?
        .response;

    let request_anchor =
        v1::lookup_request_anchor(client, &verify_presentation_request.verification_request)
            .await
            .context("lookup request anchor")?;

    let verification_material = v1::lookup_verification_materials_and_validity(
        client,
        block_identifier,
        &verify_presentation_request.presentation,
    )
    .await
    .context("lookup verification material")?;

    let verification_context = VerificationContext {
        network: state.network,
        validity_time: block_info.block_slot_time,
    };

    // Verify Presentation with respect to the Verification Request Anchor
    let presentation_verification_result: PresentationVerificationResult =
        anchor::verify_presentation_with_request_anchor(
            &global_context,
            &verification_context,
            &verify_presentation_request.verification_request,
            &verify_presentation_request.presentation,
            &request_anchor,
            &verification_material,
        );

    Ok(presentation_verification_result)
}

/// Creates and submits the Verification Audit anchor to the chain.
/// Only submits the anchor if the presenation verification result was a success.
async fn create_and_submit_audit_anchor(
    client: &mut v2::Client,
    verify_presentation_request: &VerifyPresentationRequest,
    state: &State<Arc<Service>>,
    verification_result: PresentationVerificationResult,
    account_sequence_number: concordium_rust_sdk::types::Nonce,
) -> Result<PresentationVerificationData, CreateAnchorError> {
    // Prepare Data for Audit anchor submission
    let verify_presentation_request_clone = verify_presentation_request.clone();
    let audit_record = VerificationAuditRecord::new(
        verify_presentation_request_clone.audit_record_id,
        verify_presentation_request_clone.verification_request,
        verify_presentation_request_clone.presentation,
    );

    // submit the audit anchor transaction
    let anchor_transaction_hash = if verification_result.is_success() {
        let audit_record_argument =
            build_audit_record(state, verify_presentation_request, account_sequence_number).await;

        let txn_hash = v1::submit_verification_audit_record_anchor(
            client,
            audit_record_argument.audit_record_anchor_transaction_metadata,
            &audit_record,
            audit_record_argument.public_info,
        )
        .await?;
        Some(txn_hash)
    } else {
        None
    };

    Ok(PresentationVerificationData {
        verification_result,
        audit_record,
        anchor_transaction_hash,
    })
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

    AuditRecordArgument {
        audit_record_id: verify_presentation_request.audit_record_id.clone(),
        public_info: verify_presentation_request.public_info.clone(),
        audit_record_anchor_transaction_metadata,
    }
}

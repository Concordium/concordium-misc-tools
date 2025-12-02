//! Handler for create-verification-request endpoint.
use crate::types::{ServerError, Service, VerificationRequestParams};
use axum::{Json, extract::State};
use concordium_rust_sdk::{
    base::web3id::v1::anchor::VerificationRequest,
    v2::{QueryError, RPCError},
    web3id::v1::CreateAnchorError::Query,
    web3id::v1::create_verification_request_and_submit_request_anchor,
    {
        base::web3id::v1::anchor::{
            IdentityProviderDid, RequestedIdentitySubjectClaimsBuilder,
            UnfilledContextInformationBuilder, VerificationRequestDataBuilder,
        },
        common::types::TransactionTime,
        web3id::v1::AnchorTransactionMetadata,
    },
};
use std::sync::Arc;

pub async fn create_verification_request(
    State(state): State<Arc<Service>>,
    Json(params): Json<VerificationRequestParams>,
) -> Result<Json<VerificationRequest>, ServerError> {
    let statement = RequestedIdentitySubjectClaimsBuilder::default()
        .issuers(
            params
                .issuers
                .iter()
                .map(|issuer| IdentityProviderDid::new(*issuer, state.network)),
        )
        .statements(params.statements)
        .sources(params.credential_types)
        .build();

    let context = UnfilledContextInformationBuilder::new_simple(
        params.nonce,
        params.connection_id,
        params.context_string,
    )
    .build();

    let verification_request_data = VerificationRequestDataBuilder::new(context)
        .subject_claim(statement)
        .build();

    // Transaction should expiry after some seconds.
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

    let mut node_client = state.node_client.clone();

    // Get the current nonce for the backend wallet and lock it. This is necessary
    // since it is possible that API requests come in parallel. The nonce is
    // increased by 1 and its lock is released after the transaction is submitted to
    // the blockchain.
    let mut account_sequence_number = state.nonce.lock().await;

    let anchor_transaction_metadata = AnchorTransactionMetadata {
        signer: &state.account_keys,
        sender: state.account_keys.address,
        account_sequence_number: *account_sequence_number,
        expiry,
    };

    let verification_request = create_verification_request_and_submit_request_anchor(
        &mut node_client,
        anchor_transaction_metadata,
        verification_request_data,
        Some(params.public_info),
    )
    .await;

    match verification_request {
        Ok(verification_request) => {
            // If the submission of the anchor transaction was successful,
            // increase the account_sequence_number tracked in this service.
            *account_sequence_number = account_sequence_number.next();

            Ok(Json(verification_request))
        }

        Err(e) => {
            // If the error is due to an account sequence number mismatch,
            // refresh the value in the state.
            if let Query(QueryError::RPCError(RPCError::CallError(ref err))) = e {
                if err.message() == "Duplicate nonce" || err.message() == "Nonce too large" {
                    let nonce_response = node_client
                        .get_next_account_sequence_number(&state.account_keys.address)
                        .await
                        .map_err(|e| ServerError::SubmitAnchorTransaction(e.into()))?;
                    *account_sequence_number = nonce_response.nonce;

                    return Err(ServerError::NonceMismatch(e));
                }
            }
            Err(ServerError::SubmitAnchorTransaction(e))
        }
    }
}

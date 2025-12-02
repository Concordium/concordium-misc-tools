//! Handlers for create-verification-request endpoint.
use crate::service::Service;
use axum::extract::State;
use concordium_rust_sdk::web3id::v1::create_verification_request_and_submit_request_anchor;
use concordium_rust_sdk::{
    base::web3id::v1::anchor::{
        self, IdentityCredentialType, IdentityProviderDid, RequestedIdentitySubjectClaimsBuilder,
        RequestedStatement, UnfilledContextInformationBuilder, VerificationRequestDataBuilder,
    },
    common::types::TransactionTime,
    id::id_proof_types::AttributeInRangeStatement,
    web3id::{Web3IdAttribute, v1::AnchorTransactionMetadata},
};
use std::{collections::HashMap, marker::PhantomData, sync::Arc};

pub async fn submit_anchor(State(mut state): State<Arc<Service>>) -> Result<String, String> {
    // Note: Future PRs will change these values to be posted to the service.
    let nonce_bytes: [u8; 32] = [0u8; 32];
    let context_nonce = anchor::Nonce(nonce_bytes);
    let connection_id = "MyWalletConnectTopic".to_string();
    let context_string = "MyGreateApp".to_string();
    let statements = vec![RequestedStatement::AttributeInRange(
        AttributeInRangeStatement {
            attribute_tag: 17.into(),
            lower: Web3IdAttribute::Numeric(80),
            upper: Web3IdAttribute::Numeric(1237),
            _phantom: PhantomData,
        },
    )];
    let credential_types = vec![IdentityCredentialType::IdentityCredential];
    let issuers = vec![0u32];

    let statement = RequestedIdentitySubjectClaimsBuilder::default()
        .issuers(
            issuers
                .iter()
                .map(|issuer| IdentityProviderDid::new(*issuer, state.network)),
        )
        .statements(statements)
        .sources(credential_types)
        .build();

    let context =
        UnfilledContextInformationBuilder::new_simple(context_nonce, connection_id, context_string)
            .build();

    let verification_request_data = VerificationRequestDataBuilder::new(context)
        .subject_claim(statement)
        .build();

    let public_info = HashMap::new();

    // Transaction should expiry after some seconds.
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

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

    let mut node_client = state.node_client.clone();

    let verification_request = create_verification_request_and_submit_request_anchor(
        &mut node_client,
        anchor_transaction_metadata,
        verification_request_data,
        Some(public_info.clone()),
    )
    .await;

    match verification_request {
        Ok(verification_request) => {
            *account_sequence_number = account_sequence_number.next();

            // Ok(verification_request.into())
        }
        Err(e) => {
            tracing::error!("Create anchor transaction failed {e}.");

            // TODO: If out of `nonce` error.
            // Err(ServerError::SubmitSponsoredTransactionError(e))
        }
    }
    // TODO: release lock and update service state

    Ok("create-verification-request".to_owned())
}

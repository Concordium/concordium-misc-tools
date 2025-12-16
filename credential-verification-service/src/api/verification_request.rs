//! Handler for create-verification-request endpoint.

use crate::api::util::QueryErrorExt;
use crate::node_client::NodeClient;
use crate::types::AppJson;
use crate::{
    api_types::CreateVerificationRequest,
    types::{ServerError, Service},
};
use anyhow::Context;
use axum::{Json, extract::State};
use concordium_rust_sdk::base::transactions::{BlockItem, ExactSizeTransactionSigner, send};
use concordium_rust_sdk::base::web3id::v1::anchor::{ContextLabel, VerificationRequestData};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::types::RegisteredData;
use concordium_rust_sdk::web3id::v1::CreateAnchorError;
use concordium_rust_sdk::{
    base::web3id::v1::anchor::{
        LabeledContextProperty, UnfilledContextInformationBuilder, VerificationRequest,
        VerificationRequestDataBuilder,
    },
    common::types::TransactionTime,
    web3id::v1::AnchorTransactionMetadata,
};
use std::collections::HashMap;
use std::sync::Arc;

pub async fn create_verification_request(
    State(state): State<Arc<Service>>,
    AppJson(params): AppJson<CreateVerificationRequest>,
) -> Result<Json<VerificationRequest>, ServerError> {
    let context = UnfilledContextInformationBuilder::new()
        .given(LabeledContextProperty::Nonce(params.nonce))
        .given(LabeledContextProperty::ConnectionId(params.connection_id))
        .given(LabeledContextProperty::ResourceId(params.resource_id))
        .given(LabeledContextProperty::ContextString(params.context_string))
        .requested(ContextLabel::BlockHash)
        .build();

    let mut builder = VerificationRequestDataBuilder::new(context);
    for claim in params.requested_claims {
        builder = builder.subject_claim(claim);
    }
    let verification_request_data = builder.build();

    // Transaction should expire after some seconds.
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

    let verification_request_result = create_verification_request_and_submit_request_anchor(
        &mut *node_client,
        anchor_transaction_metadata,
        verification_request_data.clone(),
        params.public_info.clone(),
    )
    .await;

    match verification_request_result {
        Ok(verification_request) => {
            // If the submission of the anchor transaction was successful,
            // increase the account_sequence_number tracked in this service.
            *account_sequence_number = account_sequence_number.next();
            Ok(Json(verification_request))
        }
        Err(CreateAnchorError::Query(err)) if err.is_account_sequence_number_error() => {
            // If the error is due to an account sequence number mismatch,
            // refresh the value in the state and try to resubmit the transaction.

            tracing::warn!(
                "Unable to submit transaction on-chain successfully due to account nonce mismatch. Account nonce will be refreshed and transaction will be re-submitted: {}",
                err
            );

            // Refresh nonce
            let nonce = node_client
                .get_next_account_sequence_number(&state.account_keys.address)
                .await
                .context("get next account sequence number")?;

            *account_sequence_number = nonce;

            tracing::info!("Refreshed account nonce successfully.");

            // Retry anchor transaction.
            let meta = AnchorTransactionMetadata {
                signer: &state.account_keys,
                sender: state.account_keys.address,
                account_sequence_number: nonce,
                expiry,
            };

            let verification_request = create_verification_request_and_submit_request_anchor(
                &mut *node_client,
                meta,
                verification_request_data,
                params.public_info,
            )
            .await
            .context("create and submit request anchor")?;

            tracing::info!(
                "Successfully submitted anchor transaction after the account nonce was refreshed."
            );

            *account_sequence_number = account_sequence_number.next();

            Ok(Json(verification_request))
        }
        Err(err) => Err(anyhow::Error::from(err)
            .context("create and submit request anchor")
            .into()),
    }
}

async fn create_verification_request_and_submit_request_anchor<S: ExactSizeTransactionSigner>(
    client: &mut dyn NodeClient,
    anchor_transaction_metadata: AnchorTransactionMetadata<S>,
    verification_request_data: VerificationRequestData,
    public_info: Option<HashMap<String, cbor::value::Value>>,
) -> Result<VerificationRequest, CreateAnchorError> {
    let verification_request_anchor = verification_request_data.to_anchor(public_info);
    let cbor = cbor::cbor_encode(&verification_request_anchor)?;
    let register_data = RegisteredData::try_from(cbor)?;

    let tx = send::register_data(
        &anchor_transaction_metadata.signer,
        anchor_transaction_metadata.sender,
        anchor_transaction_metadata.account_sequence_number,
        anchor_transaction_metadata.expiry,
        register_data,
    );
    let block_item = BlockItem::AccountTransaction(tx);

    let transaction_hash = client.send_block_item(&block_item).await?;

    Ok(VerificationRequest {
        context: verification_request_data.context,
        subject_claims: verification_request_data.subject_claims,
        anchor_transaction_hash: transaction_hash,
    })
}

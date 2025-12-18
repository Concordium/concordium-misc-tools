//! Handler for the verification endpoints.

use crate::api::util;
use crate::api_types::VerificationFailure;
use crate::node_client::NodeClient;
use crate::types::AppJson;
use crate::{
    api_types::{VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse},
    types::{ServerError, Service},
};
use anyhow::{Context, anyhow};
use axum::{Json, extract::State};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    self, PresentationVerificationResult, VerificationAuditRecord, VerificationContext,
};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    CredentialValidityType, VerifiablePresentationV1, VerificationMaterial,
    VerificationMaterialWithValidity, VerificationRequest, VerificationRequestAnchor,
    VerificationRequestAnchorAndBlockHash,
};
use concordium_rust_sdk::base::web3id::v1::{
    AccountCredentialVerificationMaterial, CredentialMetadataTypeV1, CredentialMetadataV1,
    IdentityCredentialVerificationMaterial,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::id::types;
use concordium_rust_sdk::id::types::{AccountCredentialWithoutProofs, ArInfos};
use concordium_rust_sdk::types::{
    AccountTransactionDetails, AccountTransactionEffects, BlockItemSummaryDetails,
};
use concordium_rust_sdk::v2::{BlockIdentifier, Upward};
use futures_util::future;
use std::collections::BTreeMap;
use std::sync::Arc;

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
    let presentation_verification_result = verify_presentation_with_request_anchor(
        &mut *client,
        block_identifier,
        &verify_presentation_request,
        &state,
    )
    .await?;

    // Create the audit record
    let verification_audit_record = VerificationAuditRecord::new(
        verify_presentation_request.audit_record_id,
        verify_presentation_request.verification_request,
        verify_presentation_request.presentation,
    );

    // Submit the audit anchor if verification was successful
    let anchor_transaction_hash = if presentation_verification_result.is_success() {
        let audit_record_anchor =
            verification_audit_record.to_anchor(verify_presentation_request.public_info);
        let anchor_data = util::anchor_to_registered_data(&audit_record_anchor)?;

        let txn_hash = state
            .transaction_submitter
            .submit_register_data_txn(anchor_data)
            .await?;

        Some(txn_hash)
    } else {
        None
    };

    let result = match presentation_verification_result {
        PresentationVerificationResult::Verified => VerificationResult::Verified,
        PresentationVerificationResult::Failed(code) => {
            VerificationResult::Failed(VerificationFailure {
                code,
                message: code.to_string(),
            })
        }
    };

    let verify_presentation_response = VerifyPresentationResponse {
        result,
        anchor_transaction_hash,
        verification_audit_record,
    };

    Ok(Json(verify_presentation_response))
}

/// Perform the full verification of the presentation with respect to the Verification Request Anchor.
/// This allows us to break out the verification and audit anchor submission into separate functionality
/// so that we only lock the account nonce for the anchor transaction submission, and not during the verification
/// process that this function follows.
async fn verify_presentation_with_request_anchor(
    client: &mut dyn NodeClient,
    block_identifier: BlockIdentifier,
    verify_presentation_request: &VerifyPresentationRequest,
    state: &State<Arc<Service>>,
) -> Result<PresentationVerificationResult, ServerError> {
    let global_context = client
        .get_cryptographic_parameters(block_identifier)
        .await
        .context("get cryptographic parameters")?;

    let block_slot_time = client
        .get_block_slot_time(block_identifier)
        .await
        .context("get block slot time")?;

    let request_anchor =
        lookup_request_anchor(client, &verify_presentation_request.verification_request).await?;

    let verification_material = lookup_verification_materials_and_validity(
        client,
        block_identifier,
        &verify_presentation_request.presentation,
    )
    .await?;

    let verification_context = VerificationContext {
        network: state.network,
        validity_time: block_slot_time,
    };

    // Verify Presentation with respect to the Verification Request Anchor
    let presentation_verification_result = anchor::verify_presentation_with_request_anchor(
        &global_context,
        &verification_context,
        &verify_presentation_request.verification_request,
        &verify_presentation_request.presentation,
        &request_anchor,
        &verification_material,
    );

    Ok(presentation_verification_result)
}

async fn lookup_request_anchor(
    client: &mut dyn NodeClient,
    verification_request: &VerificationRequest,
) -> Result<VerificationRequestAnchorAndBlockHash, ServerError> {
    // Fetch the transaction
    let item_status = client
        .get_block_item_status(&verification_request.anchor_transaction_hash)
        .await
        .map_err(|err| {
            if err.is_not_found() {
                ServerError::RequestAnchorTransactionNotFound(
                    verification_request.anchor_transaction_hash,
                )
            } else {
                anyhow!(err)
                    .context("get anchor transaction block item status")
                    .into()
            }
        })?;

    let (block_hash, summary) =
        item_status
            .is_finalized()
            .ok_or(ServerError::RequestAnchorTransactionNotFinalized(
                verification_request.anchor_transaction_hash,
            ))?;

    // Extract data registered payload
    let Upward::Known(BlockItemSummaryDetails::AccountTransaction(AccountTransactionDetails {
        effects: Upward::Known(AccountTransactionEffects::DataRegistered { data }),
        ..
    })) = &summary.details
    else {
        return Err(ServerError::RequestAnchorTransactionNotRegisterData(
            verification_request.anchor_transaction_hash,
        ));
    };

    // Decode anchor hash
    let verification_request_anchor: VerificationRequestAnchor = cbor::cbor_decode(data.as_ref())
        .map_err(|err| {
        ServerError::RequestAnchorDecode(verification_request.anchor_transaction_hash, err)
    })?;

    Ok(VerificationRequestAnchorAndBlockHash {
        verification_request_anchor,
        block_hash: *block_hash,
    })
}

async fn lookup_verification_materials_and_validity(
    client: &mut dyn NodeClient,
    block_identifier: BlockIdentifier,
    presentation: &VerifiablePresentationV1,
) -> Result<Vec<VerificationMaterialWithValidity>, ServerError> {
    let verification_material =
        future::try_join_all(presentation.metadata().map(|cred_metadata| {
            let mut client = client.box_clone();
            async move {
                lookup_verification_material_and_validity(
                    &mut *client,
                    block_identifier,
                    &cred_metadata,
                )
                .await
            }
        }))
        .await?;
    Ok(verification_material)
}

/// Lookup verification material for presentation
async fn lookup_verification_material_and_validity(
    client: &mut dyn NodeClient,
    block_identifier: BlockIdentifier,
    cred_metadata: &CredentialMetadataV1,
) -> Result<VerificationMaterialWithValidity, ServerError> {
    Ok(match &cred_metadata.cred_metadata {
        CredentialMetadataTypeV1::Account(metadata) => {
            let account_credentials = client
                .get_account_credentials(metadata.cred_id, block_identifier)
                .await
                .map_err(|err| {
                    if err.is_not_found() {
                        ServerError::AccountCredentialNotFound(Box::new(metadata.cred_id))
                    } else {
                        anyhow!(err).context("get account credentials").into()
                    }
                })?;

            let Some(account_cred) = account_credentials.values().find_map(|cred| {
                cred.value
                    .as_ref()
                    .known()
                    .and_then(|c| (c.cred_id() == metadata.cred_id.as_ref()).then_some(c))
            }) else {
                return Err(ServerError::AccountCredentialNotFound(Box::new(
                    metadata.cred_id,
                )));
            };

            match account_cred {
                AccountCredentialWithoutProofs::Initial { .. } => {
                    return Err(ServerError::AccountCredentialNotFound(Box::new(
                        metadata.cred_id,
                    )));
                }
                AccountCredentialWithoutProofs::Normal { cdv, commitments } => {
                    let credential_validity = types::CredentialValidity {
                        created_at: account_cred.policy().created_at,
                        valid_to: cdv.policy.valid_to,
                    };

                    VerificationMaterialWithValidity {
                        verification_material: VerificationMaterial::Account(
                            AccountCredentialVerificationMaterial {
                                issuer: cdv.ip_identity,
                                attribute_commitments: commitments.cmm_attributes.clone(),
                            },
                        ),
                        validity: CredentialValidityType::ValidityPeriod(credential_validity),
                    }
                }
            }
        }
        CredentialMetadataTypeV1::Identity(metadata) => {
            let ip_info = client
                .get_identity_providers(block_identifier)
                .await
                .context("get identity providers")?
                .into_iter()
                .find(|ip| ip.ip_identity == metadata.issuer)
                .ok_or(ServerError::IdentityProviderNotFound(metadata.issuer))?;

            let ars_infos: BTreeMap<_, _> = client
                .get_anonymity_revokers(block_identifier)
                .await
                .context("get anonymity revokers")?
                .into_iter()
                .map(|ar_info| (ar_info.ar_identity, ar_info))
                .collect();

            VerificationMaterialWithValidity {
                verification_material: VerificationMaterial::Identity(
                    IdentityCredentialVerificationMaterial {
                        ip_info,
                        ars_infos: ArInfos {
                            anonymity_revokers: ars_infos,
                        },
                    },
                ),
                validity: CredentialValidityType::ValidityPeriod(metadata.validity.clone()),
            }
        }
    })
}

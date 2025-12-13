//! Handler for the verification endpoints.

use crate::api::util::QueryErrorExt;
use crate::node_client::NodeClient;
use crate::types::AppJson;
use crate::{
    api_types::{VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse},
    types::{ServerError, Service},
};
use anyhow::Context;
use axum::{Json, extract::State};
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::transactions::{BlockItem, ExactSizeTransactionSigner, send};
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

use concordium_rust_sdk::id::types::{AccountCredentialWithoutProofs, ArInfos};
use concordium_rust_sdk::types::{
    AccountTransactionEffects, BlockItemSummaryDetails, RegisteredData,
};

use concordium_rust_sdk::id::types;
use concordium_rust_sdk::v2::BlockIdentifier;
use concordium_rust_sdk::web3id::v1::CreateAnchorError;
use concordium_rust_sdk::{
    base::web3id::v1::anchor::{
        self, PresentationVerificationResult, VerificationAuditRecord, VerificationContext,
    },
    common::types::TransactionTime,
    types::WalletAccount,
    web3id::v1::{
        AnchorTransactionMetadata, AuditRecordArgument, PresentationVerificationData, VerifyError,
    },
};
use futures_util::future;
use std::collections::{BTreeMap, HashMap};
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
    // note: we do not lock for the account nonce until the anchor submission
    let presentation_verification_result = verify_presentation_with_request_anchor(
        &mut *client,
        block_identifier,
        &verify_presentation_request,
        &state,
    )
    .await?;

    // Lock for account nonce now before we call to create and submit the audit anchor
    let mut account_sequence_number = state.nonce.lock().await;

    // create and submit the audit anchor on chain
    let presentation_verification_data_result = create_and_submit_audit_anchor(
        &mut *client,
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
            let nonce = client
                .get_next_account_sequence_number(&state.account_keys.address)
                .await
                .context("get next account sequence number")?;

            // resubmit the audit anchor with the updated account sequence number
            *account_sequence_number = nonce;
            tracing::info!("Refreshed account nonce successfully.");
            let presentation_verification_data_result = create_and_submit_audit_anchor(
                &mut *client,
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
        lookup_request_anchor(client, &verify_presentation_request.verification_request)
            .await
            .context("lookup request anchor")?;

    let verification_material = lookup_verification_materials_and_validity(
        client,
        block_identifier,
        &verify_presentation_request.presentation,
    )
    .await
    .context("lookup verification material")?;

    let verification_context = VerificationContext {
        network: state.network,
        validity_time: block_slot_time,
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

async fn lookup_request_anchor(
    client: &mut dyn NodeClient,
    verification_request: &VerificationRequest,
) -> Result<VerificationRequestAnchorAndBlockHash, VerifyError> {
    // Fetch the transaction
    let item_status = client
        .get_block_item_status(&verification_request.anchor_transaction_hash)
        .await?;

    let (block_hash, summary) = item_status
        .is_finalized()
        .ok_or(VerifyError::RequestAnchorNotFinalized)?;

    // Extract account transaction
    let BlockItemSummaryDetails::AccountTransaction(anchor_tx) =
        summary.details.as_ref().known_or_err()?
    else {
        return Err(VerifyError::InvalidRequestAnchor);
    };

    // Extract data registered payload
    let AccountTransactionEffects::DataRegistered { data } =
        anchor_tx.effects.as_ref().known_or_err()?
    else {
        return Err(VerifyError::InvalidRequestAnchor);
    };

    // Decode anchor hash
    let verification_request_anchor: VerificationRequestAnchor = cbor::cbor_decode(data.as_ref())?;

    Ok(VerificationRequestAnchorAndBlockHash {
        verification_request_anchor,
        block_hash: *block_hash,
    })
}

async fn lookup_verification_materials_and_validity(
    client: &mut dyn NodeClient,
    block_identifier: BlockIdentifier,
    presentation: &VerifiablePresentationV1,
) -> Result<Vec<VerificationMaterialWithValidity>, VerifyError> {
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
) -> Result<VerificationMaterialWithValidity, VerifyError> {
    Ok(match &cred_metadata.cred_metadata {
        CredentialMetadataTypeV1::Account(metadata) => {
            let (account_credentials, account_address) = client
                .get_account_credentials(metadata.cred_id, block_identifier)
                .await?;

            let Some(account_cred) = account_credentials.values().find_map(|cred| {
                cred.value
                    .as_ref()
                    .known()
                    .and_then(|c| (c.cred_id() == metadata.cred_id.as_ref()).then_some(c))
            }) else {
                return Err(VerifyError::CredentialNotPresent {
                    cred_id: metadata.cred_id,
                    account: account_address,
                });
            };

            match account_cred {
                AccountCredentialWithoutProofs::Initial { .. } => {
                    return Err(VerifyError::InitialCredential {
                        cred_id: metadata.cred_id,
                    });
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
                .await?
                .into_iter()
                .find(|ip| ip.ip_identity == metadata.issuer)
                .ok_or(VerifyError::UnknownIdentityProvider(metadata.issuer))?;

            let ars_infos: BTreeMap<_, _> = client
                .get_anonymity_revokers(block_identifier)
                .await?
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

/// Creates and submits the Verification Audit anchor to the chain.
/// Only submits the anchor if the presentation verification result was a success.
async fn create_and_submit_audit_anchor(
    client: &mut dyn NodeClient,
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

        let txn_hash = submit_verification_audit_record_anchor(
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

async fn submit_verification_audit_record_anchor<S: ExactSizeTransactionSigner>(
    client: &mut dyn NodeClient,
    anchor_transaction_metadata: AnchorTransactionMetadata<S>,
    verification_audit_record: &VerificationAuditRecord,
    public_info: Option<HashMap<String, cbor::value::Value>>,
) -> Result<TransactionHash, CreateAnchorError> {
    let verification_audit_anchor = verification_audit_record.to_anchor(public_info);
    let cbor = cbor::cbor_encode(&verification_audit_anchor)?;
    let register_data = RegisteredData::try_from(cbor)?;

    let tx = send::register_data(
        &anchor_transaction_metadata.signer,
        anchor_transaction_metadata.sender,
        anchor_transaction_metadata.account_sequence_number,
        anchor_transaction_metadata.expiry,
        register_data,
    );
    let item = BlockItem::AccountTransaction(tx);

    // Submit the transaction to the chain.
    let transaction_hash = client.send_block_item(&item).await?;

    Ok(transaction_hash)
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

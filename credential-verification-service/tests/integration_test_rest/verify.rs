use crate::integration_test_helpers::{fixtures, server};
use assert_matches::assert_matches;
use concordium_rust_sdk::base::sigma_protocols::common;
use concordium_rust_sdk::base::web3id::v1::CredentialVerificationMaterial;
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::{base, constants};
use credential_verification_service::api_types::{
    VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse,
};
use reqwest::StatusCode;

/// Test verify identity based presentation
#[tokio::test]
async fn test_verify_identity_based() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();

    let request_anchor_txn_hash = fixtures::chain::generate_txn_hash();
    let verification_request = fixtures::verification_request(request_anchor_txn_hash);
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);
    let verifiable_presentation_request =
        fixtures::verification_request_to_verifiable_presentation_request_identity(
            &id_cred,
            &verification_request,
        );
    let presentation = fixtures::generate_presentation_identity(
        &global_context,
        &id_cred,
        verifiable_presentation_request,
    );

    let verify_request = VerifyPresentationRequest {
        audit_record_id: "auditrecid1".to_string(),
        public_info: Some(fixtures::public_info()),
        presentation,
        verification_request,
    };

    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockInfo");
        then.pb(fixtures::chain::block_info());
    });

    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockItemStatus");
        then.pb(fixtures::chain::data_registration_block_item_finalized(
            request_anchor_txn_hash,
            vec![0u8; 1].try_into().unwrap(),
        ));
    });


    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetCryptographicParameters");
        then.pb(fixtures::chain::cryptographic_parameters(&global_context));
    });

    let verification_material = assert_matches!(&id_cred.verification_material, CredentialVerificationMaterial::Identity(ver_mat) => ver_mat);
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetIdentityProviders");
        then.pb_stream([fixtures::chain::map_ip_info(&verification_material.ip_info)].into_iter());
    });

    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetAnonymityRevokers");
        then.pb_stream(
            verification_material
                .ars_infos
                .anonymity_revokers
                .values()
                .map(fixtures::chain::map_ar_info),
        );
    });

    let audit_anchor_txn_hash = fixtures::chain::generate_txn_hash();
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/SendBlockItem");
        then.pb(generated::TransactionHash::from(&audit_anchor_txn_hash));
    });

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verify_response: VerifyPresentationResponse = resp.json().await.unwrap();
    assert_eq!(verify_response.result, VerificationResult::Verified);
    assert_eq!(
        verify_response.anchor_transaction_hash,
        Some(audit_anchor_txn_hash)
    );
}

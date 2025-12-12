use crate::integration_test_helpers::{fixtures, server};
use assert_matches::assert_matches;
use concordium_rust_sdk::base::sigma_protocols::common;
use concordium_rust_sdk::base::web3id::v1::CredentialVerificationMaterial;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    VerificationRequestAnchor, VerificationRequestData, VerificationRequestDataBuilder,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::{base, constants};
use credential_verification_service::api_types::{
    VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse,
};
use reqwest::StatusCode;

/// Test verify identity based presentation
#[tokio::test]
async fn test_verify() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let account_cred = fixtures::credentials::account_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request(&global_context, &account_cred);

    fixtures::node::stub_common(handle.node_stub(), &global_context);

    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockItemStatus");
        then.pb(fixtures::chain::data_registration_block_item_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ));
    });

    let verification_material = assert_matches!(&account_cred.verification_material, CredentialVerificationMaterial::Account(ver_mat) => ver_mat);
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetAccountInfo");
        then.pb(fixtures::chain::account_info(
            &verification_material.issuer,
            &verification_material.attribute_commitments,
        ))
        .headers([("blockhash", hex::encode(fixtures::chain::BLOCK_HASH))]);
    });

    let audit_anchor_txn_hash = fixtures::chain::generate_txn_hash();
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/SendBlockItem");
        then.pb(generated::TransactionHash::from(&audit_anchor_txn_hash));
    });

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
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

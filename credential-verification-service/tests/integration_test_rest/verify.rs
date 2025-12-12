use crate::integration_test_helpers::{fixtures, server};
use assert_matches::assert_matches;
use concordium_rust_sdk::base::web3id::v1::CredentialVerificationMaterial;
use credential_verification_service::api_types::{VerificationResult, VerifyPresentationResponse};
use reqwest::StatusCode;

/// Test verify account based presentation
#[tokio::test]
async fn test_verify_account_based() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let account_cred = fixtures::credentials::account_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_account(&global_context, &account_cred);

    let verification_material = assert_matches!(&account_cred.verification_material, CredentialVerificationMaterial::Account(ver_mat) => ver_mat);

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
}

/// Test verify account based presentation
#[tokio::test]
async fn test_verify_identity_based() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    let verification_material = assert_matches!(&id_cred.verification_material, CredentialVerificationMaterial::Identity(ver_mat) => ver_mat);

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
}

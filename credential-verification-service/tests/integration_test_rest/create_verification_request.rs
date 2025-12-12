use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::base::web3id::v1::anchor::VerificationRequest;
use reqwest::StatusCode;

/// Test create verification request
#[tokio::test]
async fn test_create_verification_request() {
    let handle = server::start_server();

    let create_request = fixtures::create_verification_request();

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let _verification_request: VerificationRequest = resp.json().await.unwrap();
}

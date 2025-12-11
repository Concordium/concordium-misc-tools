use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::v2::generated;
use credential_verification_service::api_types::{
    VerifyPresentationRequest, VerifyPresentationResponse,
};
use reqwest::StatusCode;

/// Test verify identity based presentation
#[tokio::test]
async fn test_verify_identity_based() {
    let handle = server::start_server();

    let verification_request = fixtures::verification_request();

    let presentation = todo!();

    let verify_request = VerifyPresentationRequest {
        audit_record_id: "auditrecid1".to_string(),
        public_info: Some(fixtures::public_info()),
        presentation,
        verification_request,
    };

    let txn_hash = fixtures::generate_txn_hash();
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/SendBlockItem");
        then.pb(generated::TransactionHash::from(&txn_hash));
    });

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verify_response: VerifyPresentationResponse = resp.json().await.unwrap();
}

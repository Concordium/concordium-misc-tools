use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::base::web3id::v1::anchor::{VerificationRequest, VerificationRequestData};
use concordium_rust_sdk::common::cbor;
use credential_verification_service::api_types::{
    VerificationResult, VerifyPresentationRequest, VerifyPresentationResponse,
};
use reqwest::StatusCode;

/// Test create verification request, generate presentation for it, and then verify.
#[tokio::test]
async fn test_create_verification_request_and_verify() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let create_request = fixtures::create_verification_request();

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verification_request: VerificationRequest = resp.json().await.unwrap();

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

    let verification_data = VerificationRequestData {
        context: verification_request.context.clone(),
        subject_claims: verification_request.subject_claims.clone(),
    };
    let request_anchor = verification_data.to_anchor(None);
    handle.node_client_stub().stub_block_item_status(
        verification_request.anchor_transaction_hash,
        fixtures::chain::transaction_status_finalized(
            verification_request.anchor_transaction_hash,
            cbor::cbor_encode(&request_anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

    let verify_request = VerifyPresentationRequest {
        audit_record_id: "recid1".to_string(),
        public_info: None,
        presentation,
        verification_request,
    };

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
}

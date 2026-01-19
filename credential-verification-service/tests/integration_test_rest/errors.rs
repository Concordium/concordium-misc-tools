use crate::integration_test_helpers::{fixtures, node_client_mock, server};
use concordium_rust_sdk::{
    base::{
        hashes::TransactionHash,
        web3id::v1::anchor::{RequestedStatement, RequestedSubjectClaims},
    },
    id::{
        constants::AttributeKind, id_proof_types::AttributeInRangeStatement, types::AttributeTag,
    },
    web3id::Web3IdAttribute,
};
use credential_verification_service::api_types::ErrorResponse;
use reqwest::StatusCode;
use std::marker::PhantomData;

pub const ATTRIBUTE_TAG_NATIONALITY: AttributeTag = AttributeTag(5);

/// Test where JSON send in request is not valid.
#[tokio::test]
async fn test_invalid_json_in_request() {
    let handle = server::start_server();

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json("notvalid")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let text = resp.text().await.unwrap();
    assert!(text.contains("invalid json"), "test: {}", text);
}

fn make_invalid_range_statement() -> RequestedStatement<AttributeTag> {
    RequestedStatement::AttributeInRange(AttributeInRangeStatement {
        attribute_tag: ATTRIBUTE_TAG_NATIONALITY,
        lower: Web3IdAttribute::String(AttributeKind::try_new("1".into()).unwrap()),
        upper: Web3IdAttribute::String(AttributeKind::try_new("2".into()).unwrap()),
        _phantom: PhantomData,
    })
}

/// Test request payload validation error
#[tokio::test]
async fn test_request_payload_validation_error() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let mut verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    for claim in &mut verify_fixture.request.verification_request.subject_claims {
        let RequestedSubjectClaims::Identity(id_claim) = claim;
        id_claim.statements = vec![make_invalid_range_statement()];
    }

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    let status = resp.status();
    let body_text = resp.text().await.unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // Map to client friendly error structure
    let error_response_body: ErrorResponse = serde_json::from_str(&body_text).unwrap();

    // expected field values in error response
    let expected_error_code = "VALIDATION_ERROR".to_string();
    let expected_top_level_error_message = "Validation errors have occurred. Please check the details below for more information.".to_string();
    let expected_error_detail_code = "ATTRIBUTE_IN_RANGE_STATEMENT_INVALID_ATTRIBUTE_TAG".to_string();
    let expected_error_detail_message = "Attribute tag `nationality` is not allowed to be used in range statements. Only `ATTRIBUTE_TAG_DOB(3)`, `ATTRIBUTE_TAG_ID_DOC_ISSUED_AT(9)`, and `ATTRIBUTE_TAG_ID_DOC_EXPIRES_AT(10)` allowed in range statements.".to_string();
    
    // assertions
    assert_eq!(error_response_body.error.code, expected_error_code);
    assert_eq!(error_response_body.error.message, expected_top_level_error_message);
    assert_eq!(error_response_body.error.retryable, false);
    
    // ensure we have one corresponding error detail
    assert_eq!(1, error_response_body.error.details.len());
    let error_detail = &error_response_body.error.details[0];
    assert_eq!(error_detail.code, expected_error_detail_code);
    assert_eq!(error_detail.message, expected_error_detail_message);
}

/// Test internal server error
#[tokio::test]
async fn test_internal_error() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let mut verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);
    verify_fixture
        .request
        .verification_request
        .anchor_transaction_hash =
        TransactionHash::from(node_client_mock::GET_BLOCK_ITEM_FAIL_TXN_HASH);

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

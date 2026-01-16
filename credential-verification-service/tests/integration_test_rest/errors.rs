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

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let text = resp.text().await.unwrap();
    assert!(
        text.contains("Attribute tag `nationality` is not allowed to be used in range statements."),
        "test: {}",
        text
    );
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

    let text = resp.text().await.unwrap();
    assert_eq!(text, "internal server error");
}

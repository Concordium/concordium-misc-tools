use crate::integration_test_helpers::{fixtures, server};
use assert_matches::assert_matches;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    IdentityCredentialType, IdentityProviderDid, PresentationVerifyFailure,
    RequestedIdentitySubjectClaims, RequestedStatement,
};
use concordium_rust_sdk::base::web3id::v1::{
    CredentialVerificationMaterial, anchor::RequestedSubjectClaims,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::id::types::IpIdentity;
use concordium_rust_sdk::web3id::did::Network;
use credential_verification_service::api_types::{
    ErrorResponse, VerificationResult, VerifyPresentationResponse,
};
use credential_verification_service::validation::validation_context::{
    VALIDATION_GENERAL_ERROR_CODE, VALIDATION_GENERAL_MESSAGE,
};
use reqwest::StatusCode;

/// Test verify account based presentation
#[tokio::test]
async fn test_verify_account_based() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let account_cred = fixtures::credentials::account_credentials_fixture(&global_context, 1);

    let verify_fixture = fixtures::verify_request_account(&global_context, &account_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

    let verification_material = assert_matches!(&account_cred.verification_material, CredentialVerificationMaterial::Account(ver_mat) => ver_mat);

    handle.node_client_stub().stub_account_credentials(
        account_cred.cred_id,
        fixtures::chain::account_credentials(
            &account_cred.cred_id,
            verification_material.issuer,
            verification_material.attribute_commitments.clone(),
        ),
    );

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
    handle.node_client_stub().expect_send_block_item(
        &verify_response
            .anchor_transaction_hash
            .expect("anchor should be submitted"),
    );
    assert_eq!(
        verify_response.verification_audit_record.id,
        verify_fixture.request.audit_record_id
    );
    assert_eq!(
        verify_response.verification_audit_record.request,
        verify_fixture.request.verification_request
    );
    assert_eq!(
        verify_response.verification_audit_record.presentation,
        verify_fixture.request.presentation
    );
}

/// Test verify identity based presentation
#[tokio::test]
async fn test_verify_identity_based() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

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
    handle.node_client_stub().expect_send_block_item(
        &verify_response
            .anchor_transaction_hash
            .expect("anchor should be submitted"),
    );
    assert_eq!(
        verify_response.verification_audit_record.id,
        verify_fixture.request.audit_record_id
    );
    assert_eq!(
        verify_response.verification_audit_record.request,
        verify_fixture.request.verification_request
    );
    assert_eq!(
        verify_response.verification_audit_record.presentation,
        verify_fixture.request.presentation
    );
}

/// Test anchor not finalized
#[tokio::test]
async fn test_verify_anchor_status_committed() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_committed(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

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
    handle.node_client_stub().expect_send_block_item(
        &verify_response
            .anchor_transaction_hash
            .expect("anchor should be submitted"),
    );
    assert_eq!(
        verify_response.verification_audit_record.id,
        verify_fixture.request.audit_record_id
    );
    assert_eq!(
        verify_response.verification_audit_record.request,
        verify_fixture.request.verification_request
    );
    assert_eq!(
        verify_response.verification_audit_record.presentation,
        verify_fixture.request.presentation
    );
}

/// Test verification that fails
#[tokio::test]
async fn test_verify_fail() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let mut verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

    // make presentation not match the request
    verify_fixture
        .request
        .presentation
        .presentation_context
        .requested
        .clear();

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verify_response: VerifyPresentationResponse = resp.json().await.unwrap();
    assert_matches!(verify_response.result, VerificationResult::Failed(failure) => {
        assert_eq!(failure.code, PresentationVerifyFailure::NoVraBlockHash);
        assert_eq!(failure.message, "verification request anchor block hash not set in context");
    });
    assert!(verify_response.anchor_transaction_hash.is_none());
    assert_eq!(
        verify_response.verification_audit_record.id,
        verify_fixture.request.audit_record_id
    );
    assert_eq!(
        verify_response.verification_audit_record.request,
        verify_fixture.request.verification_request
    );
    assert_eq!(
        verify_response.verification_audit_record.presentation,
        verify_fixture.request.presentation
    );
}

/// Test anchor not CBOR decodable
#[tokio::test]
async fn test_verify_anchor_not_decodable() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            vec![0].try_into().unwrap(),
        ),
    );

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Assertions and expected validation codes and messages
    let expected_code = "REQUEST_ANCHOR_DECODE_ISSUE";
    let expected_message = format!(
        "request anchor transaction {} encountered a decoding issue.",
        verify_fixture
            .request
            .verification_request
            .anchor_transaction_hash
    );

    assert_eq!(expected_code, error_response.error.code);
    assert_eq!(expected_message, error_response.error.message);
    assert!(!error_response.error.retryable);
}

/// Test request anchor not found
#[tokio::test]
async fn test_verify_anchor_not_found() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Assertions and expected validation codes and messages
    let expected_code = "REQUEST_ANCHOR_NOT_FOUND";
    let expected_message = format!(
        "request anchor transaction {} not found",
        verify_fixture
            .request
            .verification_request
            .anchor_transaction_hash
    );

    assert_eq!(expected_code, error_response.error.code);
    assert_eq!(expected_message, error_response.error.message);
    assert!(!error_response.error.retryable);
}

/// Test account credential not found
#[tokio::test]
async fn test_verify_account_credential_not_found() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let account_cred = fixtures::credentials::account_credentials_fixture(&global_context, 2);

    let verify_fixture = fixtures::verify_request_account(&global_context, &account_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Assertions and expected validation codes and messages
    let expected_code = "ACCOUNT_CREDENTIAL_NOT_FOUND";
    let expected_message = "Account credential could not be found: a64b6991952b448c57dc1d04c78bd255a2387b49eda3e493e7935d9203cda3a321ae80bd00165915491ac4e157b88b1d";

    assert_eq!(expected_code, error_response.error.code);
    assert_eq!(expected_message, error_response.error.message);
    assert!(!error_response.error.retryable);
}

/// Multiple statements provided, range and set. both invalid
#[tokio::test]
async fn test_verify_identity_based_multistatement_both_invalid() {
    let handle = server::start_server();
    let global_context = fixtures::credentials::global_context();
    let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

    let mut verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

    handle.node_client_stub().stub_block_item_status(
        verify_fixture.anchor_txn_hash,
        fixtures::chain::transaction_status_finalized(
            verify_fixture.anchor_txn_hash,
            cbor::cbor_encode(&verify_fixture.anchor)
                .unwrap()
                .try_into()
                .unwrap(),
        ),
    );

    // valid range statement
    let attribute_in_range_statement =
        RequestedStatement::AttributeInRange(fixtures::make_range_statement("19900101", "abcd"));

    // invalid set statement. UK is not valid it has to be GB
    let set_statement =
        RequestedStatement::AttributeInSet(fixtures::make_country_set_statement(vec!["UK"]));

    verify_fixture.request.verification_request.subject_claims =
        vec![RequestedSubjectClaims::Identity(
            RequestedIdentitySubjectClaims {
                statements: vec![attribute_in_range_statement, set_statement],
                issuers: vec![IdentityProviderDid {
                    identity_provider: IpIdentity(0u32),
                    network: Network::Testnet,
                }],
                source: vec![IdentityCredentialType::IdentityCredential],
            },
        )];

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/verify")
        .json(&verify_fixture.request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Validation Error response assertions.
    // There should be 1 detail, not retryable (as its a validation error)
    assert_eq!(error_response.error.code, VALIDATION_GENERAL_ERROR_CODE);
    assert_eq!(error_response.error.message, VALIDATION_GENERAL_MESSAGE);
    assert!(!error_response.error.retryable);
    assert_eq!(error_response.error.details.len(), 2);

    fixtures::assert_has_detail(
        &error_response.error.details,
        "COUNTRY_CODE_INVALID",
        "Country code must be 2 letter and both uppercase following the ISO3166-1 alpha-2 uppercase standard. (e.g `DE`)",
        "verificationRequest.subjectClaims[0].statements[1].set[0]",
    );

    fixtures::assert_has_detail(
        &error_response.error.details,
        "ATTRIBUTE_IN_RANGE_STATEMENT_NOT_NUMERIC",
        "Attribute in range statement, is a numeric range check between a lower and upper bound. These must be numeric values.",
        "verificationRequest.subjectClaims[0].statements[0].upper",
    );
}

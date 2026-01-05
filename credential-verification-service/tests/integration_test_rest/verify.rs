use crate::integration_test_helpers::{fixtures, server};
use assert_matches::assert_matches;
use concordium_rust_sdk::base::web3id::v1::CredentialVerificationMaterial;
use concordium_rust_sdk::base::web3id::v1::anchor::PresentationVerifyFailure;
use concordium_rust_sdk::common::cbor;
use credential_verification_service::api_types::{VerificationResult, VerifyPresentationResponse};
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

/// Test verify account based presentation
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

// TODO: rethink how to simulate the process of the transaction getting finalized over time
// /// Test anchor not finalized
// #[tokio::test]
// async fn test_verify_anchor_not_finalized() {
//     let handle = server::start_server();
//     let global_context = fixtures::credentials::global_context();
//     let id_cred = fixtures::credentials::identity_credentials_fixture(&global_context);

//     let verify_fixture = fixtures::verify_request_identity(&global_context, &id_cred);

//     handle.node_client_stub().stub_block_item_status(
//         verify_fixture.anchor_txn_hash,
//         fixtures::chain::transaction_status_committed(
//             verify_fixture.anchor_txn_hash,
//             cbor::cbor_encode(&verify_fixture.anchor)
//                 .unwrap()
//                 .try_into()
//                 .unwrap(),
//         ),
//     );

//     let resp = handle
//         .rest_client()
//         .post("verifiable-presentations/verify")
//         .json(&verify_fixture.request)
//         .send()
//         .await
//         .unwrap();

//     assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
//     let resp_text = resp.text().await.unwrap();
//     assert!(
//         resp_text.contains("request anchor transaction") && resp_text.contains("not finalized"),
//         "response: {}",
//         resp_text
//     );
// }

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
    let resp_text = resp.text().await.unwrap();
    assert!(
        resp_text.contains("error decoding registered data"),
        "response: {}",
        resp_text
    );
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
    let resp_text = resp.text().await.unwrap();
    assert!(
        resp_text.contains("request anchor transaction") && resp_text.contains("not found"),
        "response: {}",
        resp_text
    );
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
    let resp_text = resp.text().await.unwrap();
    assert!(
        resp_text.contains("account credential") && resp_text.contains("not found"),
        "response: {}",
        resp_text
    );
}

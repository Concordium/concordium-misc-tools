use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::common::cbor;
use futures_util::{FutureExt, future};
use reqwest::StatusCode;

/// Test send a lot of requests. Tests can nonce management is correct and
/// can handle multiple requests.
#[tokio::test]
async fn test_many_requests() {
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

    const REQUEST_COUNT: usize = 10;

    let verify_request = verify_fixture.request.clone();
    let verify_requests = (0..REQUEST_COUNT).map(|_| {
        async {
            let resp = handle
                .rest_client()
                .post("verifiable-presentations/verify")
                .json(&verify_request)
                .send()
                .await
                .expect("verify_request");
            assert_eq!(resp.status(), StatusCode::OK);
        }
        .boxed()
    });

    let failing_verify_request = {
        let mut failing_verify_request = verify_fixture.request.clone();
        failing_verify_request
            .presentation
            .presentation_context
            .requested
            .clear();
        failing_verify_request
    };
    let failing_verify_requests = (0..REQUEST_COUNT).map(|_| {
        async {
            let resp = handle
                .rest_client()
                .post("verifiable-presentations/verify")
                .json(&failing_verify_request)
                .send()
                .await
                .expect("failing_verify_request");
            assert_eq!(resp.status(), StatusCode::OK);
        }
        .boxed()
    });

    let create_verification_request = fixtures::create_verification_request();
    let create_verification_requests = (0..REQUEST_COUNT).map(|_| {
        async {
            let resp = handle
                .rest_client()
                .post("verifiable-presentations/create-verification-request")
                .json(&create_verification_request)
                .send()
                .await
                .expect("create_verification_request");
            assert_eq!(resp.status(), StatusCode::OK);
        }
        .boxed()
    });

    future::join_all(
        verify_requests
            .chain(failing_verify_requests)
            .chain(create_verification_requests),
    )
    .await;
}

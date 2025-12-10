use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::v2::generated;
use reqwest::StatusCode;
use wallet_proxy_api::{ErrorCode, ErrorResponse};

/// Test request to node fails. Should result in
/// internal error code.
#[tokio::test]
async fn test_node_request_fails() {
    let handle = server::start_server();

    let txn_hash = fixtures::generate_txn_hash();

    handle.node_mock().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockItemStatus")
            .pb(generated::TransactionHash::from(&txn_hash));
        then.internal_server_error();
    });

    let resp = handle
        .rest_client()
        .get(format!("v0/submissionStatus/{}", txn_hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let err_resp: ErrorResponse = resp.json().await.unwrap();
    assert_eq!(err_resp.error, ErrorCode::Internal);
    assert!(
        err_resp.error_message.contains("RPC error"),
        "message: {}",
        err_resp.error_message
    );
}

/// Test where parsing request fails because path parameter
/// fails.
#[tokio::test]
async fn test_invalid_request_path_parameter() {
    let handle = server::start_server();

    // invalid transaction hash
    let txn_hash = "aaa";

    let resp = handle
        .rest_client()
        .get(format!("v0/submissionStatus/{}", txn_hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let err_resp: ErrorResponse = resp.json().await.unwrap();
    assert_eq!(err_resp.error, ErrorCode::InvalidRequest);
    assert!(
        err_resp.error_message.contains("invalid path parameters"),
        "message: {}",
        err_resp.error_message
    );
}

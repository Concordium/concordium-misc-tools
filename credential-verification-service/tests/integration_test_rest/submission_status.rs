use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::v2::generated;
use reqwest::StatusCode;
use wallet_proxy_api::{SubmissionStatus, TransactionStatus};

#[tokio::test]
async fn test_submission_status() {
    let handle = server::start_server();

    let txn_hash = fixtures::generate_txn_hash();

    handle.node_mock().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockItemStatus")
            .pb(generated::TransactionHash::from(&txn_hash));
        then.pb(generated::BlockItemStatus {
            status: Some(generated::block_item_status::Status::Received(
                Default::default(),
            )),
        });
    });

    let resp = handle
        .rest_client()
        .get(format!("v0/submissionStatus/{}", txn_hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let submission_status: SubmissionStatus = resp.json().await.unwrap();
    assert_eq!(submission_status.status, TransactionStatus::Received);
}

#[tokio::test]
async fn test_submission_status_absent() {
    let handle = server::start_server();

    let txn_hash = fixtures::generate_txn_hash();

    handle.node_mock().mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockItemStatus")
            .pb(generated::TransactionHash::from(&txn_hash));
        then.not_found().message("not found");
    });

    let resp = handle
        .rest_client()
        .get(format!("v0/submissionStatus/{}", txn_hash))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let submission_status: SubmissionStatus = resp.json().await.unwrap();
    assert_eq!(submission_status.status, TransactionStatus::Absent);
}

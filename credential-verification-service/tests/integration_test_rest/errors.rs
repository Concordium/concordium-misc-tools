use crate::integration_test_helpers::server;
use reqwest::StatusCode;

/// Test where JSON send in request is not valid.
#[tokio::test]
async fn test_invalid_json_in_request() {
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
    assert_eq!(
        "aa",
        String::from_utf8_lossy(resp.bytes().await.unwrap().as_ref()),
    );
}

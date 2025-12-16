use crate::integration_test_helpers::server;
use reqwest::StatusCode;

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

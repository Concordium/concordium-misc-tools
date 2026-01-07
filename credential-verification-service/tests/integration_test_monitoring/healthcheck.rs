//! Tests healthcheck endpoint

use crate::integration_test_helpers::server;
use reqwest::StatusCode;
use serde_json::value;

/// Test healthcheck endpoint
#[tokio::test]
async fn test_healthcheck() {
    let handle = server::start_server();

    let resp = handle
        .monitoring_client()
        .get("health")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let _body: value::Value = resp.json().await.unwrap();
}

//! Test metrics endpoint

use crate::integration_test_helpers::server;
use reqwest::StatusCode;

/// Test scraping metrics
#[tokio::test]
async fn test_prometheus_metrics_scrape() {
    let handle = server::start_server();

    let resp = handle
        .monitoring_client()
        .get("metrics")
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
}

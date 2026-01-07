//! Handlers for the monitoring endpoints.
use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;
use crate::types::Service;
use std::sync::Arc;
use prometheus_client::registry::Registry;

#[derive(Clone)]
pub struct MonitoringState {
    pub registry: Arc<Registry>,
    pub service: Arc<Service>,
}

/// GET Handler for route `/metrics`.
/// Exposes the metrics in the registry in the Prometheus format.
pub async fn metrics(State(metrics_registry): State<MonitoringState>) -> Result<String, String> {
    let mut buffer = String::new();

    let registry = metrics_registry.registry;

    prometheus_client::encoding::text::encode(&mut buffer, &registry)
        .map_err(|err| err.to_string())?;
    Ok(buffer)
}

/// GET Handler for route `/health`.
/// Verifying the API service state is as expected.
pub async fn health(State(_service): State<MonitoringState>) -> (StatusCode, Json<serde_json::Value>) {
    let healthy = {
        // TODO: implement actual checks
        true
    };
    if healthy {
        (StatusCode::OK, Json(json!("Ok")))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Not Ok")))
    }
}

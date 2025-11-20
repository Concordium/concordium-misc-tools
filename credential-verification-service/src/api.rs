use axum::{Router, routing::get};
use prometheus_client::registry::Registry;
use std::sync::Arc;

use crate::service::Service;

mod monitroing;
mod verifier;

/// Router exposing the service's endpoints
pub fn router(service: Arc<Service>) -> Router {
    Router::new()
        .route("/verify", get(verifier::verify))
        .with_state(service)
}

/// Router exposing the Prometheus metrics and health endpoint.
pub fn monitoring_router(metrics_registry: Registry, service: Arc<Service>) -> Router {
    let metric_routes = Router::new()
        .route("/", get(monitroing::metrics))
        .with_state(Arc::new(metrics_registry));
    let health_routes = Router::new()
        .route("/", get(monitroing::health))
        .with_state(service);

    Router::new()
        .nest("/metrics", metric_routes)
        .nest("/health", health_routes)
}

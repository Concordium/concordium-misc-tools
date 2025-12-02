use axum::{
    Router,
    routing::{get, post},
};
use prometheus_client::registry::Registry;
use std::sync::Arc;

use crate::service::Service;

mod common;
mod monitoring;
mod verification_request;
mod verifier;

/// Router exposing the service's endpoints
pub fn router(service: Arc<Service>) -> Router {
    Router::new()
        .route("/verifiable-presentations/verify", post(verifier::verify))
        .route(
            "/verifiable-presentations/create-verification-request",
            post(verification_request::create_verification_request),
        )
        .with_state(service)
}

/// Router exposing the Prometheus metrics and health endpoint.
pub fn monitoring_router(metrics_registry: Registry, service: Arc<Service>) -> Router {
    let metric_routes = Router::new()
        .route("/", get(monitoring::metrics))
        .with_state(Arc::new(metrics_registry));
    let health_routes = Router::new()
        .route("/", get(monitoring::health))
        .with_state(service);

    Router::new()
        .nest("/metrics", metric_routes)
        .nest("/health", health_routes)
}

use crate::api::monitoring::MonitoringState;
use crate::types::Service;
use axum::{
    Router,
    routing::{get, post},
};
use prometheus_client::registry::Registry;
use std::sync::Arc;

mod create_verification_request;
pub mod middleware;
mod monitoring;
mod util;
mod verify;

/// Router exposing the service's endpoints
pub fn router(service: Arc<Service>, request_timeout: u64) -> Router {
    Router::new()
        .route(
            "/verifiable-presentations/verify",
            post(verify::verify_presentation),
        )
        .route(
            "/verifiable-presentations/create-verification-request",
            post(create_verification_request::create_verification_request),
        )
        .with_state(service)
        .layer(tower_http::timeout::TimeoutLayer::new(
            std::time::Duration::from_millis(request_timeout),
        ))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(1_000_000)) // at most 1000kB of data.
        .layer(tower_http::compression::CompressionLayer::new())
}

/// Router exposing the Prometheus metrics and health endpoint.
pub fn monitoring_router(metrics_registry: Registry) -> Router {
    let state = MonitoringState {
        registry: Arc::new(metrics_registry),
    };

    let metric_routes = Router::new()
        .route("/", get(monitoring::metrics))
        .with_state(state.clone());

    let health_routes = Router::new()
        .route("/", get(monitoring::health))
        .with_state(state.clone());

    Router::new()
        .nest("/metrics", metric_routes)
        .nest("/health", health_routes)
}

use std::sync::Arc;

use axum::{Router, routing::get};

use crate::service::Service;

mod health;

pub fn init_routes(service: Service) -> Router {
    Router::new()
        .route("/health", get(health::health))
        .with_state(Arc::new(service))
}

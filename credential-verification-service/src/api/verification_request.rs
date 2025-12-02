//! Handlers for create-verification-request endpoint.
use crate::{api::common::submit_anchor, service::Service};
use axum::{Json, extract::State};
use std::sync::Arc;

pub async fn create_verification_request(
    state: State<Arc<Service>>,
    Json(_payload): Json<bool>,
) -> Json<String> {
    // TODO: return the value
    let _ = submit_anchor(state).await;

    Json("ok".to_string())
}

//! Handler for the verification endpoints.
use crate::types::Service;
use axum::{Json, extract::State};
use std::sync::Arc;

pub async fn verify(_state: State<Arc<Service>>, Json(_payload): Json<bool>) -> Json<String> {
    Json("ok".to_string())
}

use std::sync::Arc;

use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;

use crate::service::Service;

pub async fn health(State(service): State<Arc<Service>>) -> (StatusCode, Json<serde_json::Value>) {
    if service.healthy {
        (StatusCode::OK, Json(json!("Ok")))
    } else {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(json!("Not Ok")))
    }
}

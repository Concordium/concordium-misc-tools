use crate::api::monitoring::MonitoringState;
use crate::types::Service;
use axum::{
    Router, http,
    extract::{ConnectInfo, State},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use prometheus_client::registry::Registry;
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

mod create_verification_request;
pub mod middleware;
mod monitoring;
mod util;
mod verify;

/// Shared API security configuration and state: authentication plus rate limiting.
#[derive(Clone)]
pub struct ApiSecurity {
    /// API key required in the `X-Api-Key` header. `None` disables authentication
    /// (legacy/local-dev mode; a startup warning is emitted by the service).
    pub api_key: Option<Arc<str>>,
    /// Maximum requests per client IP within `rate_limit_window`.
    pub rate_limit_max: u32,
    /// Length of the sliding per-IP rate limit window.
    pub rate_limit_window: Duration,
    /// Explicitly allowed CORS origins for browser requests. Empty means
    /// cross-origin browser requests are not allowed.
    pub allowed_origins: Arc<Vec<String>>,
    /// Per-IP sliding-window request timestamps.
    windows: Arc<Mutex<HashMap<IpAddr, Vec<Instant>>>>,
}

impl ApiSecurity {
    pub fn new(
        api_key: Option<String>,
        rate_limit_max: u32,
        rate_limit_window: Duration,
        allowed_origins: Vec<String>,
    ) -> Self {
        Self {
            api_key: api_key.map(Arc::from),
            rate_limit_max,
            rate_limit_window,
            allowed_origins: Arc::new(allowed_origins),
            windows: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

/// Middleware enforcing API key authentication (when configured) and a simple
/// per-IP sliding-window rate limit. This protects the endpoints that submit
/// on-chain anchor transactions from unauthenticated flooding.
async fn api_security_middleware(
    State(security): State<ApiSecurity>,
    connect_info: Option<ConnectInfo<SocketAddr>>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if let Some(api_key) = &security.api_key {
        let provided = request
            .headers()
            .get("x-api-key")
            .and_then(|v| v.to_str().ok());
        match provided {
            Some(provided) if provided == api_key.as_ref() => {}
            _ => {
                return (http::StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
            }
        }
    }

    if let Some(ConnectInfo(addr)) = connect_info {
        let now = Instant::now();
        let mut windows = security.windows.lock().expect("rate limiter lock poisoned");
        let requests = windows.entry(addr.ip()).or_default();
        requests.retain(|ts| now.duration_since(*ts) < security.rate_limit_window);
        if requests.len() >= security.rate_limit_max as usize {
            return (http::StatusCode::TOO_MANY_REQUESTS, "Too many requests").into_response();
        }
        requests.push(now);
    }

    next.run(request).await
}

/// Router exposing the service's endpoints
pub fn router(service: Arc<Service>, request_timeout: u64, security: ApiSecurity) -> Router {
    // Build a restrictive CORS layer: only explicitly configured origins may make
    // cross-origin browser requests. A permissive policy would let any website
    // drive anchor submissions from a visitor's browser.
    let origins = security
        .allowed_origins
        .iter()
        .filter_map(|origin| origin.parse::<http::HeaderValue>().ok())
        .collect::<Vec<_>>();
    let cors_layer = tower_http::cors::CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([http::Method::POST])
        .allow_headers([
            http::header::CONTENT_TYPE,
            http::header::HeaderName::from_static("x-api-key"),
        ]);

    Router::new()
        .route(
            "/verifiable-presentations/verify",
            post(verify::verify_presentation),
        )
        .route(
            "/verifiable-presentations/create-verification-request",
            post(create_verification_request::create_verification_request),
        )
        .route_layer(axum::middleware::from_fn_with_state(
            security,
            api_security_middleware,
        ))
        .with_state(service)
        .layer(tower_http::timeout::TimeoutLayer::new(
            std::time::Duration::from_millis(request_timeout),
        ))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(1_000_000)) // at most 1000kB of data.
        .layer(cors_layer)
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

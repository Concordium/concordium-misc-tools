use anyhow::Context;
use axum::{
    http::{self, StatusCode},
    routing::get,
    Router,
};
use clap::Parser;
use concordium_rust_sdk::{
    endpoints::QueryError,
    smart_contracts::common::AccountAddress,
    v2::{self, BlockIdentifier},
};
use futures::{stream::FuturesOrdered, TryStreamExt};
use prometheus::{
    core::{AtomicU64, GenericGauge},
    Opts, Registry, TextEncoder,
};
use std::sync::Arc;
use tonic::transport::ClientTlsConfig;
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse};

#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(version, author)]
struct App {
    #[clap(
        long = "node",
        help = "GRPC V2 interface of the node.",
        default_value = "http://localhost:20000",
        env = "CHAIN_PROMETHEUS_EXPORTER_CONCORDIUM_NODE"
    )]
    endpoint:        v2::Endpoint,
    #[clap(
        long = "listen-address",
        default_value = "0.0.0.0:9090",
        help = "Listen address for the server.",
        env = "CHAIN_PROMETHEUS_EXPORTER_API_LISTEN_ADDRESS"
    )]
    listen_address:  std::net::SocketAddr,
    #[clap(
        long = "log-level",
        default_value = "info",
        help = "Maximum log level.",
        env = "CHAIN_PROMETHEUS_EXPORTER_LOG_LEVEL"
    )]
    log_level:       tracing_subscriber::filter::LevelFilter,
    #[clap(
        long = "log-headers",
        help = "Whether to log headers for requests and responses.",
        env = "CHAIN_PROMETHEUS_EXPORTER_LOG_HEADERS"
    )]
    log_headers:     bool,
    #[clap(
        long = "request-timeout",
        help = "Request timeout in milliseconds.",
        default_value = "5000",
        env = "CHAIN_PROMETHEUS_EXPORTER_REQUEST_TIMEOUT"
    )]
    request_timeout: u64,
    #[clap(
        long = "account",
        help = "List of account addresses to monitor.",
        env = "CHAIN_PROMETHEUS_EXPORTER_ACCOUNTS",
        value_delimiter = ','
    )]
    accounts:        Vec<String>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Unable to query node: {0}")]
    Query(#[from] QueryError),
    #[error("Unable to produce metrics: {0}")]
    Encoding(#[from] prometheus::Error),
}

impl axum::response::IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let r = match self {
            Self::Query(err) => {
                if err.is_not_found() {
                    (
                        StatusCode::NOT_FOUND,
                        axum::Json("Requested value not found.".to_string()),
                    )
                } else {
                    tracing::error!("Error querying the node: {}", err);
                    (
                        StatusCode::BAD_GATEWAY,
                        axum::Json("Cannot reach the node".into()),
                    )
                }
            }
            Error::Encoding(err) => {
                tracing::error!("Could not collect metrics: {err:#}.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    axum::Json("Could not collect metrics.".into()),
                )
            }
        };
        r.into_response()
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();

    {
        use tracing_subscriber::prelude::*;
        let log_filter = tracing_subscriber::filter::Targets::new()
            .with_target(module_path!(), app.log_level)
            .with_target("tower_http", app.log_level);
        tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer())
            .with(log_filter)
            .init();
    }

    let registry = Registry::new();
    let mut gauges = Vec::with_capacity(app.accounts.len());
    for acc in app.accounts {
        let mut iter = acc.split(':');
        let label = iter.next().context("No label")?;
        let address = iter.next().context("No address")?.parse()?;
        tracing::info!("Tracking account {address} with label {label}.");
        let opts = Opts::new(
            format!("{label}_balance"),
            &format!("Balance of account {address} in microCCD."),
        );
        let gauge: GenericGauge<AtomicU64> = GenericGauge::with_opts(opts)?;
        gauges.push((address, gauge.clone()));
        registry.register(Box::new(gauge))?;
    }

    let endpoint = if app
        .endpoint
        .uri()
        .scheme()
        .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
    {
        app.endpoint
            .tls_config(ClientTlsConfig::new())
            .context("Unable to construct TLS configuration for Concordium API.")?
    } else {
        app.endpoint
    }
    .connect_timeout(std::time::Duration::from_secs(10))
    .timeout(std::time::Duration::from_millis(app.request_timeout));

    let client = v2::Client::new(endpoint)
        .await
        .context("Unable to establish connection to the node.")?;

    // build routes
    let server = Router::new()
        .route("/metrics", get(text_metrics))
        .with_state((client, registry, Arc::new(gauges)))
        .layer(tower_http::trace::TraceLayer::new_for_http().
               make_span_with(DefaultMakeSpan::new().
                              include_headers(app.log_headers)).
               on_response(DefaultOnResponse::new().
                           include_headers(app.log_headers)))
        .layer(tower_http::timeout::TimeoutLayer::new(
            std::time::Duration::from_millis(app.request_timeout),
        ))
        .layer(tower_http::limit::RequestBodyLimitLayer::new(0)) // no bodies, we only have GET requests.
        .layer(tower_http::cors::CorsLayer::permissive().allow_methods([http::Method::GET]));

    axum::Server::bind(&app.listen_address)
        .serve(server.into_make_service())
        .await?;
    Ok(())
}

/// State maintained by the service.
type ServiceState = (
    // Connection to the node.
    v2::Client,
    // Prometheus registry
    Registry,
    // List of accounts to query, along with their gauges for recording balances.
    Arc<Vec<(AccountAddress, GenericGauge<AtomicU64>)>>,
);

#[tracing::instrument(level = "debug", skip(client, registry))]
async fn text_metrics(
    axum::extract::State((client, registry, gauges)): axum::extract::State<ServiceState>,
) -> Result<String, axum::response::ErrorResponse> {
    let mut futures = FuturesOrdered::new();
    for acc in gauges.iter().map(|x| x.0) {
        let mut client = client.clone();
        futures.push_back(async move {
            let acc = client
                .get_account_info(&acc.into(), BlockIdentifier::LastFinal)
                .await?
                .response;
            Ok::<_, QueryError>(acc.account_amount)
        })
    }
    match futures.try_collect::<Vec<_>>().await {
        Ok(balances) => {
            for (balance, gauge) in balances.into_iter().zip(gauges.iter()) {
                gauge.1.set(balance.micro_ccd())
            }
        }
        Err(e) => return Err(Error::Query(e).into()),
    }

    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    Ok(encoder
        .encode_to_string(&metric_families)
        .map_err(Error::Encoding)?)
}

use anyhow::Context;
use axum::{
    http::{self, StatusCode},
    routing::get,
    Router,
};
use clap::Parser;
use concordium_rust_sdk::{
    endpoints::QueryError,
    types::{
        hashes::{BlockHash, TransactionHash},
        queries::{BlockInfo, ConsensusInfo},
        AbsoluteBlockHeight, TransactionStatus,
    },
    v2,
};
use futures::TryStreamExt;
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
        env = "NETWORK_DASHBOARD_PROXY_CONCORDIUM_NODE"
    )]
    endpoint:        v2::Endpoint,
    #[clap(
        long = "listen-address",
        default_value = "0.0.0.0:8080",
        help = "Listen address for the server.",
        env = "NETWORK_DASHBOARD_PROXY_API_LISTEN_ADDRESS"
    )]
    listen_address:  std::net::SocketAddr,
    #[clap(
        long = "log-level",
        default_value = "info",
        help = "Maximum log level.",
        env = "NETWORK_DASHBOARD_PROXY_LOG_LEVEL"
    )]
    log_level:       tracing_subscriber::filter::LevelFilter,
    #[clap(
        long = "log-headers",
        help = "Whether to log headers for requests and responses.",
        env = "NETWORK_DASHBOARD_PROXY_LOG_HEADERS"
    )]
    log_headers:     bool,
    #[clap(
        long = "request-timeout",
        help = "Request timeout in milliseconds.",
        default_value = "5000",
        env = "NETWORK_DASHBOARD_PROXY_REQUEST_TIMEOUT"
    )]
    request_timeout: u64,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Unable to query node: {0:?}")]
    Query(#[from] QueryError),
    #[error("Network error: {0:?}")]
    Network(#[from] v2::Status),
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
            Error::Network(err) => {
                tracing::error!("Error processing the stream from the node: {}", err);
                (
                    StatusCode::BAD_GATEWAY,
                    axum::Json("Error processing stream from the node.".into()),
                )
            }
        };
        r.into_response()
    }
}

async fn transaction_status(
    axum::extract::Path(tx): axum::extract::Path<TransactionHash>,
    axum::extract::State(mut client): axum::extract::State<v2::Client>,
) -> Result<axum::Json<TransactionStatus>, Error> {
    let _span = tracing::debug_span!("transaction_status");
    tracing::debug!("Request for transaction status for {tx}.");
    let r = client.get_block_item_status(&tx).await?;
    Ok(r.into())
}

async fn blocks_by_height(
    axum::extract::Path(height): axum::extract::Path<u64>,
    axum::extract::State(mut client): axum::extract::State<v2::Client>,
) -> Result<axum::Json<Vec<BlockHash>>, Error> {
    let _span = tracing::debug_span!("blocks_by_height", height = height);
    tracing::debug!("Request for blocks at height {height}.");
    let r = client
        .get_blocks_at_height(&AbsoluteBlockHeight::from(height).into())
        .await?;
    Ok(r.into())
}

async fn block_info(
    axum::extract::Path(block_hash): axum::extract::Path<BlockHash>,
    axum::extract::State(mut client): axum::extract::State<v2::Client>,
) -> Result<axum::Json<BlockInfo>, Error> {
    let _span = tracing::debug_span!("block_info");
    tracing::debug!("Request for block info for {block_hash}.");
    let r = client.get_block_info(&block_hash).await?;
    Ok(r.response.into())
}

async fn consensus_status(
    axum::extract::State(mut client): axum::extract::State<v2::Client>,
) -> Result<axum::Json<ConsensusInfo>, Error> {
    let _span = tracing::debug_span!("consensus_status");
    tracing::debug!("Request for consensus status.");
    let r = client.get_consensus_info().await?;
    Ok(r.into())
}

async fn block_summary(
    axum::extract::Path(block_hash): axum::extract::Path<BlockHash>,
    axum::extract::State(c): axum::extract::State<v2::Client>,
) -> Result<axum::Json<serde_json::Value>, Error> {
    let _span = tracing::debug_span!("block_summary");
    tracing::debug!("Request for block summary for {block_hash}.");
    let mut client = c.clone();
    let txs = async move {
        let txs = client
            .get_block_transaction_events(block_hash)
            .await?
            .response
            .try_collect::<Vec<_>>()
            .await?;
        Ok::<_, Error>(txs)
    };
    let mut client = c.clone();
    let special = async move {
        let special = client
            .get_block_special_events(block_hash)
            .await?
            .response
            .try_collect::<Vec<_>>()
            .await?;
        Ok::<_, Error>(special)
    };
    let mut client = c.clone();
    let finalization_data = async move {
        let finalization_data = client
            .get_block_finalization_summary(block_hash)
            .await?
            .response;
        Ok::<_, Error>(finalization_data)
    };
    let mut client = c.clone();
    let pending_updates = async move {
        let pending = client
            .get_block_pending_updates(block_hash)
            .await?
            .response
            .try_collect::<Vec<_>>()
            .await?;
        Ok::<_, Error>(pending)
    };
    let mut client = c;
    let chain_parameters = async move {
        let chain_parameters = client
            .get_block_chain_parameters(block_hash)
            .await?
            .response;
        Ok::<_, Error>(chain_parameters)
    };
    let (txs, special, finalization_data, pending_updates, chain_parameters) = futures::try_join!(
        txs,
        special,
        finalization_data,
        pending_updates,
        chain_parameters
    )?;
    let reward_parameters = match chain_parameters {
        v2::ChainParameters::V0(cp) => {
            serde_json::json!({
                "mintDistribution": cp.mint_distribution,
                "transactionFeeDistribution": cp.transaction_fee_distribution,
                "gasRewards": cp.gas_rewards
            })
        }
        v2::ChainParameters::V1(cp) => {
            serde_json::json!({
                "mintDistribution": cp.mint_distribution,
                "transactionFeeDistribution": cp.transaction_fee_distribution,
                "gasRewards": cp.gas_rewards
            })
        }
    };
    Ok(serde_json::json!({
        "finalizationData": finalization_data,
        "transactionSummaries": txs,
        "specialEvents": special,
        "pendingUpdates": pending_updates,
        "chainParameters": {
            "rewardParameters": reward_parameters
        }
    })
    .into())
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
    let server = Router::new().route("/v1/transactionStatus/:transactionHash", get(transaction_status))
        .route("/v1/consensusStatus", get(consensus_status))
        .route("/v1/blockSummary/:blockHash", get(block_summary))
        .route("/v1/blockInfo/:blockHash", get(block_info))
        .route("/v1/blocksByHeight/:height", get(blocks_by_height))
        .with_state(client)
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

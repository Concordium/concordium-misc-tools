use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::put,
    Router,
};
use clap::Parser;
use dotenv::dotenv;
use std::sync::Arc;
use tokio_postgres::Config;
use tracing::info;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "NOTIFICATION_SERVER_DB_CONNECTION"
    )]
    db_connection: String,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level:     log::LevelFilter,
}

#[derive(Clone)]
struct AppState {
    db_connection: Config,
}

async fn upsert_account_device(
    Path(device): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(account): Json<Vec<String>>,
) -> Result<impl axum::response::IntoResponse, axum::response::Response> {
    info!(
        "Subscribing accounts {:?} to device {}",
        account, device
    );

    let _ = &state.db_connection;

    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let db_connection: Config = args.db_connection.parse()?;
    let app_state = Arc::new(AppState { db_connection });

    let app = Router::new()
        .route(
            "/api/v1/device/:device/subscriptions",
            put(upsert_account_device),
        )
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3030").await?;
    axum::serve(listener, app).await?;
    Ok(())
}

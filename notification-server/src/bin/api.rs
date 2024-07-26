use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    routing::put,
    Router,
};
use base64::prelude::*;
use clap::Parser;
use dotenv::dotenv;
use std::sync::Arc;
use tokio_postgres::Config;
use tracing::info;
use notification_server::database::DatabaseConnection;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "NOTIFICATION_SERVER_DB_CONNECTION"
    )]
    db_connection:  String,
    #[arg(
        long = "listen-address",
        help = "Listen address for the server.",
        env = "NOTIFICATION_SERVER_LISTEN_ADDRESS",
        default_value = "0.0.0.0:3030"
    )]
    listen_address: std::net::SocketAddr,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level:      log::LevelFilter,
}

async fn upsert_account_device(
    Path(device): Path<String>,
    State(db_connection): State<Arc<DatabaseConnection>>,
    Json(account): Json<String>,
) -> Result<impl axum::response::IntoResponse, axum::response::Response> {
    info!("Subscribing accounts {:?} to device {}", account, device);
    let decoded_account = BASE64_STANDARD.decode(device.as_bytes()).map_err(|_| StatusCode::BAD_REQUEST)?;
    let _ = &db_connection.prepared.upsert_device(&decoded_account, &account).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let db_connection: Config = args.db_connection.parse()?;

    let app_state = Arc::new(db_connection);

    // TODO add authentication middleware
    let app = Router::new()
        .route(
            "/api/v1/device/:device/subscription",
            put(upsert_account_device),
        )
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind(args.listen_address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

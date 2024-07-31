use axum::{
    extract::{Json, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::put,
    Router,
};
use backoff::ExponentialBackoff;
use clap::Parser;
use concordium_rust_sdk::base::contracts_common::{AccountAddress, AccountAddressParseError};
use dotenv::dotenv;
<<<<<<< HEAD
use notification_server::{
    database::DatabaseConnection, google_cloud::GoogleCloud, models::DeviceSubscription,
};
use std::{path, path::PathBuf, sync::Arc, time::Duration};
use anyhow::anyhow;
use gcp_auth::CustomServiceAccount;
=======
use enum_iterator::all;
use lazy_static::lazy_static;
use notification_server::{
    database::DatabaseConnection,
    models::{DeviceSubscription, Preference},
};
use serde_json::json;
use std::{collections::HashSet, str::FromStr, sync::Arc};
>>>>>>> lma/gcm_integration
use tokio_postgres::Config;
use tracing::{error, info};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "NOTIFICATION_SERVER_DB_CONNECTION"
    )]
    db_connection: Config,
    #[arg(
        long = "listen-address",
        help = "Listen address for the server.",
        env = "NOTIFICATION_SERVER_LISTEN_ADDRESS",
        default_value = "0.0.0.0:3030"
    )]
    listen_address: std::net::SocketAddr,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level: log::LevelFilter,
    #[arg(
        long = "google-application-credentials",
        help = "Credentials used for verifying the device token.",
        env = "NOTIFICATION_SERVER_GOOGLE_APPLICATION_CREDENTIALS_PATH"
    )]
    google_application_credentials_path: String,
    #[arg(
        long = "google-client-timeout-secs",
        help = "Request timeout connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_TIMEOUT_SECS",
        default_value_t = 15
    )]
    google_client_timeout_secs: u64,
    #[arg(
        long = "google-client-connection-timeout-secs",
        help = "Request connection timeout connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_CONNECTION_TIMEOUT_SECS",
        default_value_t = 5
    )]
    google_client_connection_timeout_secs: u64,
    #[arg(
        long = "google-client-max-elapsed-time-secs",
        help = "Max elapsed time for connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_MAX_ELAPSED_TIME_SECS",
        default_value_t = 20
    )]
    google_client_max_elapsed_time_secs: u64,

    #[arg(
        long = "google-client-max-interval-time-secs",
        help = "Max interval time for retries when connecting to the Google API in seconds.",
        env = "NOTIFICATION_SERVER_GOOGLE_CLIENT_MAX_INTERVAL_TIME_SECS",
        default_value_t = 3
    )]
    google_client_max_interval_time_secs: u64,
}

#[derive(Debug)]
struct AppState {
    db_connection: DatabaseConnection,
    google_cloud:  GoogleCloud<CustomServiceAccount>,
}

const MAX_RESOURCES_LENGTH: usize = 1000;

lazy_static! {
    static ref MAX_PREFERENCES_LENGTH: usize = all::<Preference>().collect::<Vec<_>>().len();
}

#[tracing::instrument]
async fn upsert_account_device(
    Path(device): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(subscription): Json<DeviceSubscription>,
) -> Result<impl IntoResponse, Response> {
    info!(
        "Subscribing accounts {:?} to device {}",
        subscription, device
    );
    if subscription.preferences.len() > *MAX_PREFERENCES_LENGTH {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Preferences exceeded length of {}", *MAX_PREFERENCES_LENGTH),
        )
            .into_response())?;
    }
    let unique_preferences: HashSet<_> = subscription.preferences.iter().collect();
    if unique_preferences.len() != subscription.preferences.len() {
        return Err((StatusCode::BAD_REQUEST, "Duplicate preferences found").into_response());
    }

    if subscription.accounts.len() > MAX_RESOURCES_LENGTH {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "Preferences exceed maximum length of {}",
                MAX_RESOURCES_LENGTH
            ),
        )
            .into_response())?;
    }

    let decoded_accounts: Result<Vec<Vec<u8>>, Response> = subscription
        .accounts
        .iter()
        .map(|account| {
            let account_address: Result<AccountAddress, AccountAddressParseError> =
                AccountAddress::from_str(account);
            account_address
                .map_err(|e| {
                    error!("Failed to parse account address: {}", e);
                    (StatusCode::BAD_REQUEST, "Failed to parse account address").into_response()
                })
                .map(|value| value.0.to_vec())
        })
        .collect();
    
    if let Err(err) = state.google_cloud.validate_device_token(&device).await {
        error!(
            "Unexpected response provided by gcm service while validating device_token: {}",
            err
        );
        return Err((StatusCode::BAD_REQUEST, "Invalid device token").into_response());
    }

    let decoded_accounts = decoded_accounts?;
    state
        .db_connection
        .prepared
        .upsert_subscription(decoded_accounts, subscription.preferences, &device)
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to write subscriptions to database",
            )
                .into_response()
        })?;
    Ok(Json(json!({"message": "Subscribed accounts to device"})))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();

    tracing_subscriber::fmt::init();

    let retry_policy = ExponentialBackoff {
        max_elapsed_time: Some(Duration::from_secs(
            args.google_client_max_elapsed_time_secs,
        )),
        max_interval: Duration::from_secs(args.google_client_max_interval_time_secs),
        ..ExponentialBackoff::default()
    };

    let http_client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(
            args.google_client_connection_timeout_secs,
        ))
        .timeout(Duration::from_secs(args.google_client_timeout_secs))
        .build()?;

    let path = PathBuf::from(args.google_application_credentials_path);
    let service_account = CustomServiceAccount::from_file(path)?;
    let project_id = service_account.project_id().ok_or(anyhow!("Project ID not found in service account"))?;
    let app_state = Arc::new(AppState {
        db_connection: DatabaseConnection::create(args.db_connection).await?,
        google_cloud:  GoogleCloud::new(
            http_client,
            retry_policy,
            service_account,
            project_id
        )?,
    });

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

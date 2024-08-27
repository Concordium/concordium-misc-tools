use anyhow::Context;
use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
    routing::put,
    Router,
};
use backoff::ExponentialBackoff;
use clap::Parser;
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use dotenv::dotenv;
use enum_iterator::all;
use gcp_auth::CustomServiceAccount;
use lazy_static::lazy_static;
use notification_server::{
    database::DatabaseConnection,
    google_cloud::{GoogleCloud, NotificationError},
    models::device::{DeviceSubscription, Preference},
};
use serde_json::json;
use std::{collections::HashSet, path::PathBuf, str::FromStr, sync::Arc, time::Duration};
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

/// Processes a device subscription by validating and updating the device's
/// accounts.
///
/// # Arguments
/// * `device` - A string identifier for the device.
/// * `subscription` - The subscription details including accounts.
/// * `state` - Shared application state
///
/// # Returns
/// Returns a `Result` indicating success with a confirmation message or an
/// error with status code and description.
///
/// # Errors
/// Returns `Err` with appropriate status code and error message for any of the
/// following conditions:
/// - Exceeding the maximum preferences length.
/// - Duplicate preferences found.
/// - Exceeding the maximum number of accounts.
/// - Parsing failure of account addresses.
/// - Device token validation failure.
/// - Database errors during subscription update.
async fn process_device_subscription(
    subscription: DeviceSubscription,
    state: Arc<AppState>,
) -> Result<String, (StatusCode, String)> {
    if subscription.preferences.len() > *MAX_PREFERENCES_LENGTH {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Preferences exceeded length of {}", *MAX_PREFERENCES_LENGTH),
        ));
    }
    let unique_preferences: HashSet<_> = subscription.preferences.iter().collect();
    if unique_preferences.len() != subscription.preferences.len() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Duplicate preferences found".to_string(),
        ));
    }
    if subscription.accounts.len() > MAX_RESOURCES_LENGTH {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("Accounts exceed maximum length of {}", MAX_RESOURCES_LENGTH),
        ));
    }

    let decoded_accounts: Result<Vec<AccountAddress>, (StatusCode, String)> = subscription
        .accounts
        .iter()
        .map(|account| {
            AccountAddress::from_str(account).map_err(|_| {
                (
                    StatusCode::BAD_REQUEST,
                    "Failed to parse account address".to_string(),
                )
            })
        })
        .collect();

    if let Err(err) = state
        .google_cloud
        .validate_device_token(&subscription.device_token)
        .await
    {
        let (status, message) = match err {
            NotificationError::InvalidArgumentError => {
                (StatusCode::BAD_REQUEST, "Invalid device token".to_string())
            }
            NotificationError::UnregisteredError => (
                StatusCode::NOT_FOUND,
                "The device token has not been registered".to_string(),
            ),
            NotificationError::AuthenticationError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Authentication towards the external messaging service failed".to_string(),
            ),
            NotificationError::ClientError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Client error received from external message service".to_string(),
            ),
            NotificationError::ServerError(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server error received from external message service".to_string(),
            ),
        };
        return Err((status, message));
    }

    let decoded_accounts = decoded_accounts?;
    state
        .db_connection
        .prepared
        .upsert_subscription(
            decoded_accounts,
            subscription.preferences,
            &subscription.device_token,
        )
        .await
        .map_err(|e| {
            error!("Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error occurred while writing subscriptions to database"
                    .to_string(),
            )
        })?;
    Ok("Subscribed accounts to device".to_string())
}

async fn upsert_account_device(
    State(state): State<Arc<AppState>>,
    Json(subscription): Json<DeviceSubscription>,
) -> impl IntoResponse {
    info!("Subscribing accounts {:?} a device token", subscription);
    let response: Result<String, (StatusCode, String)> =
        process_device_subscription(subscription, state).await;
    match response {
        Ok(message) => (StatusCode::OK, Json(json!({ "message": message }))),
        Err((status_code, message)) => {
            if status_code.is_server_error() {
                error!("Server error: {}", message);
            }
            if status_code.is_client_error() {
                info!("Invalid request: {}", message);
            }
            (status_code, Json(json!({ "errorMessage": message })))
        }
    }
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
    let project_id = service_account
        .project_id()
        .context("Project ID not found in service account")?
        .to_string();
    let app_state = Arc::new(AppState {
        db_connection: DatabaseConnection::create(args.db_connection).await?,
        google_cloud:  GoogleCloud::new(http_client, retry_policy, service_account, &project_id),
    });

    let app = Router::new()
        .route("/api/v1/subscription", put(upsert_account_device))
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind(args.listen_address).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

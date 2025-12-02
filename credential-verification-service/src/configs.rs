use clap::Parser;
use concordium_rust_sdk::v2;
use std::{net::SocketAddr, path::PathBuf};

#[derive(Parser, Debug)]
#[clap(arg_required_else_help(true))]
pub struct ServiceConfigs {
    #[arg(long, env = "CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT")]
    pub node_endpoint: v2::Endpoint,
    #[arg(
        long = "request-timeout",
        help = "Request timeout (both of request to the node and server requests) in milliseconds.",
        default_value = "10000",
        env = "CREDENTIAL_VERIFICATION_SERVICE_REQUEST_TIMEOUT"
    )]
    pub request_timeout: u64,
    #[arg(
        long,
        help = "The socket address where the service exposes its API.",
        default_value = "127.0.0.1:8000",
        env = "CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS"
    )]
    pub api_address: SocketAddr,
    #[arg(
        long,
        help = "The socket address used for health and metrics monitoring.",
        default_value = "127.0.0.1:8001",
        env = "CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS"
    )]
    pub monitoring_address: SocketAddr,
    #[arg(
        long,
        help = "Path to the wallet keys.",
        env = "CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT"
    )]
    pub account: PathBuf,
    #[arg(
        long = "transaction-expiry",
        help = "The number of seconds in the future when the anchor transactions should expiry.",
        default_value = "1000000",
        env = "CREDENTIAL_VERIFICATION_SERVICE_TRANSACTION_EXPIRY"
    )]
    pub transaction_expiry_secs: u32,
    #[arg(
        long,
        help = "The maximum log level  [`off`, `error`, `warn`, `info`, `debug`, or `trace`]",
        default_value = "info",
        env = "CREDENTIAL_VERIFICATION_SERVICE_LOG_LEVEL"
    )]
    pub log_level: tracing_subscriber::filter::LevelFilter,
}

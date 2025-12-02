use clap::Parser;
use concordium_rust_sdk::v2;

use std::{net::SocketAddr, path::PathBuf};

#[derive(Parser, Debug)]
#[clap(arg_required_else_help(true))]
pub struct ServiceConfigs {
    #[arg(long, env = "CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT")]
    pub node_endpoint: v2::Endpoint,
    #[clap(
        long = "request-timeout",
        help = "Request timeout (both of request to the node and server requests) in milliseconds.",
        default_value = "10000",
        env = "REQUEST_TIMEOUT"
    )]
    pub request_timeout: u64,
    #[arg(
        long,
        env = "CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS",
        default_value = "127.0.0.1:8000"
    )]
    pub api_address: SocketAddr,
    #[arg(
        long,
        env = "CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS",
        default_value = "127.0.0.1:8001"
    )]
    pub monitoring_address: SocketAddr,
    #[arg(
        long,
        env = "CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT",
        help = "Path to the wallet keys."
    )]
    pub account: PathBuf,
    #[arg(long, default_value = "info", env = "LOG_LEVEL")]
    pub log_level: tracing_subscriber::filter::LevelFilter,
}

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
        help = "The request timeout for a request to be processed with the credential service api in milliseconds.",
        default_value = "15000",
        env = "CREDENTIAL_VERIFICATION_SERVICE_REQUEST_TIMEOUT"
    )]
    pub request_timeout: u64,
    #[arg(
        long = "grpc-node-request-timeout",
        help = "The request timeout to the Concordium node in milliseconds",
        default_value = "1000",
        env = "CREDENTIAL_VERIFICATION_GRPC_NODE_REQUEST_TIMEOUT"
    )]
    pub grpc_node_request_timeout: u64,
    #[arg(
        long = "acquire-account-sequence-lock-timeout",
        help = "Timeout for acquiring the local lock on the account sequence number in order to submit anchor transaction",
        default_value = "5000",
        env = "CREDENTIAL_VERIFICATION_ACQUIRE_ACCOUNT_SEQUENCE_LOCK_TIMEOUT"
    )]
    pub acquire_account_sequence_lock_timeout: u64,
    #[arg(
        long = "anchor-wait-for-finalization-timeout",
        help = "Timeout for waiting for the request anchor transaction to finalize",
        default_value = "6000",
        env = "CREDENTIAL_VERIFICATION_ANCHOR_WAIT_FOR_FINALIZATION_TIMEOUT"
    )]
    pub anchor_wait_for_finalization_timeout: u64,
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
        default_value = "15",
        env = "CREDENTIAL_VERIFICATION_SERVICE_TRANSACTION_EXPIRY"
    )]
    pub transaction_expiry_secs: u32,
    #[arg(
        long,
        help = "The log level  [`off`, `error`, `warn`, `info`, `debug`, or `trace`]",
        default_value = "info",
        env = "CREDENTIAL_VERIFICATION_SERVICE_LOG_LEVEL"
    )]
    pub log_level: tracing_subscriber::filter::LevelFilter,
}

use clap::Parser;
use concordium_rust_sdk::{
    endpoints::{self, QueryError, RPCError},
    types::queries::{ActiveConsensusState, ConsensusState},
};
use thiserror::Error;

#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(version, author)]
struct App {
    #[clap(
        long = "node",
        help = "GRPC interface of the node.",
        default_value = "http://localhost:10000",
        env = "LIVENESS_CHECKER_NODE"
    )]
    endpoint:      endpoints::Endpoint,
    #[clap(
        long = "rpc-token",
        help = "GRPC interface access token for accessing the node.",
        default_value = "rpcadmin",
        env = "LIVENESS_CHECKER_TOKEN"
    )]
    token:         String,
    #[clap(
        long = "max-finalized-behind",
        help = "Maximum number of seconds the last finalized block can be behind present.",
        env = "LIVENESS_CHECKER_MAX_FINALIZED_BEHIND"
    )]
    max_behind:    i64,
    #[clap(
        long = "min-peers",
        help = "Minimum number of peers the node must have.",
        env = "LIVENESS_CHECKER_MIN_PEERS"
    )]
    min_peers:     usize,
    #[clap(
        long = "require-baker",
        help = "Require the node to be a baker.",
        env = "LIVENESS_CHECKER_REQUIRE_BAKER"
    )]
    require_baker: bool,
}

#[derive(Debug, Error)]
enum ReturnStatus {
    #[error("Failed to connect: {0}")]
    ConnectionFailed(#[from] tonic::transport::Error),
    #[error("RPC error, failed to query: {0}")]
    RPCError(#[from] RPCError),
    #[error("Query failed: {0}")]
    QueryFailed(#[from] QueryError),
    #[error("The node did not yet witness finalization.")]
    NoFinalization,
    #[error("Finalization is too far behind.")]
    FinalizationTooFarBehind,
    #[error("The node does not have enough peers.")]
    TooFewPeers,
    #[error("Not a baker.")]
    NotABaker,
}

async fn worker() -> Result<(), ReturnStatus> {
    let app = App::parse();

    let endpoint_with_timeout = app
        .endpoint
        .connect_timeout(std::time::Duration::from_secs(2))
        .timeout(std::time::Duration::from_secs(5));
    let mut client = endpoints::Client::connect(endpoint_with_timeout, app.token).await?;

    let consensus = client.get_consensus_status().await?;

    if consensus.last_finalized_time.is_none() {
        return Err(ReturnStatus::NoFinalization);
    }

    if app.require_baker {
        let node_info = client.node_info().await?;
        match node_info.peer_details {
            concordium_rust_sdk::types::queries::PeerDetails::Bootstrapper => {
                return Err(ReturnStatus::NotABaker);
            }
            concordium_rust_sdk::types::queries::PeerDetails::Node {
                consensus_state,
            } => {
                if let ConsensusState::Active {
                    active_state,
                } = consensus_state
                {
                    if let ActiveConsensusState::Active {
                        ..
                    } = active_state
                    {
                    } else {
                        return Err(ReturnStatus::NotABaker);
                    }
                } else {
                    return Err(ReturnStatus::NotABaker);
                }
            }
        }
    }

    let peer_list = client.peer_list(false).await?;
    if peer_list.len() < app.min_peers {
        return Err(ReturnStatus::TooFewPeers);
    }

    let lfb = consensus.last_finalized_block;
    let lfb_info = client.get_block_info(&lfb).await?;
    if chrono::Utc::now().signed_duration_since(lfb_info.block_slot_time).num_seconds()
        > app.max_behind
    {
        return Err(ReturnStatus::FinalizationTooFarBehind);
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = worker().await {
        match &e {
            ReturnStatus::ConnectionFailed(_) => {
                eprintln!("{:?}", e);
                std::process::exit(1);
            }
            ReturnStatus::RPCError(_) => {
                eprintln!("{:?}", e);
                std::process::exit(2);
            }
            ReturnStatus::QueryFailed(_) => {
                eprintln!("{:?}", e);
                std::process::exit(3);
            }
            ReturnStatus::NoFinalization => {
                eprintln!("{:?}", e);
                std::process::exit(4);
            }
            ReturnStatus::FinalizationTooFarBehind => {
                eprintln!("{:?}", e);
                std::process::exit(5);
            }
            ReturnStatus::TooFewPeers => {
                eprintln!("{:?}", e);
                std::process::exit(6);
            }
            ReturnStatus::NotABaker => {
                eprintln!("{:?}", e);
                std::process::exit(7);
            }
        }
    }
}

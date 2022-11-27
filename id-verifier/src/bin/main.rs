use clap::Parser;
use concordium_rust_sdk::{
    endpoints::{QueryError, RPCError},
    id::{
        constants::{ArCurve, AttributeKind},
        id_proof_types::{Proof, StatementWithContext},
        range_proof::RangeProof,
        types::{AccountAddress, AccountCredentialWithoutProofs, GlobalContext},
    },
    types::CredentialRegistrationID,
    v2::BlockIdentifier,
};
use log::{error, info, warn};
use std::{
    collections::HashMap,
    convert::Infallible,
    sync::{Arc, Mutex},
};

use rand::Rng;
use warp::{http::StatusCode, Filter, Rejection, Reply};

/// Structure used to receive the correct command line arguments.
#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true))]
#[clap(version, author)]
struct IdVerifierConfig {
    #[clap(
        long = "node",
        help = "GRPC V2 interface of the node.",
        default_value = "http://localhost:20000"
    )]
    endpoint:  concordium_rust_sdk::v2::Endpoint,
    #[clap(
        long = "port",
        default_value = "8100",
        help = "Port on which the server will listen on."
    )]
    port:      u16,
    #[structopt(
        long = "log-level",
        default_value = "debug",
        help = "Maximum log level."
    )]
    log_level: log::LevelFilter,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct AgeProofOutput {
    account: AccountAddress,
    lower:   AttributeKind,
    upper:   AttributeKind,
    proof:   RangeProof<ArCurve>,
}

type Challenge = [u8; 32];

#[derive(Clone)]
struct Server {
    statement_map:  Arc<Mutex<HashMap<Challenge, StatementWithContext<ArCurve, AttributeKind>>>>,
    global_context: Arc<GlobalContext<ArCurve>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app = IdVerifierConfig::parse();
    let mut log_builder = env_logger::Builder::new();
    // only log the current module (main).
    log_builder.filter_level(app.log_level); // filter filter_module(module_path!(), app.log_level);
    log_builder.init();

    let mut client = concordium_rust_sdk::v2::Client::new(app.endpoint).await?;
    let global_context = client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;

    log::debug!("Acquired data from the node.");

    let state = Server {
        statement_map:  Arc::new(Mutex::new(HashMap::new())),
        global_context: Arc::new(global_context),
    };
    let add_state = state.clone();
    let prove_state = state.clone();

    // 1. Inject statement
    let inject_statement = warp::post()
        .and(warp::filters::body::content_length_limit(50 * 1024))
        .and(warp::path!("inject"))
        .and(handle_inject_statement(add_state));

    // 2. Provide proof
    let provide_proof = warp::post()
        .and(warp::filters::body::content_length_limit(50 * 1024))
        .and(warp::path!("prove"))
        .and(handle_provide_proof(client, prove_state));

    info!("Starting up HTTP server. Listening on port {}.", app.port);
    let cors = warp::cors()
        .allow_any_origin()
        .allow_header("Content-Type")
        .allow_method("POST");

    let server = inject_statement
        .or(provide_proof)
        .recover(handle_rejection)
        .with(cors)
        .with(warp::trace::request());
    warp::serve(server).run(([0, 0, 0, 0], app.port)).await;
    Ok(())
}

fn handle_inject_statement(
    state: Server,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::body::json().and_then(
        move |request: StatementWithContext<ArCurve, AttributeKind>| {
            let state = state.clone();
            async move {
                log::debug!("Parsed statement. Generating challenge");
                match inject_statement_worker(state, request).await {
                    Ok(r) => Ok(warp::reply::json(&r)),
                    Err(e) => {
                        warn!("Request is invalid {:#?}.", e);
                        Err(warp::reject::custom(e))
                    }
                }
            }
        },
    )
}

fn handle_provide_proof(
    client: concordium_rust_sdk::v2::Client,
    state: Server,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::body::json().and_then(move |request: ChallengedProof| {
        let client = client.clone();
        let state = state.clone();
        async move {
            info!("Queried for injecting statement");
            match check_proof_worker(client, state, request).await {
                Ok(r) => Ok(warp::reply::json(&r)),
                Err(e) => {
                    warn!("Request is invalid {:#?}.", e);
                    Err(warp::reject::custom(e))
                }
            }
        }
    })
}

#[derive(Debug)]
/// An internal error type used by this server to manage error handling.
#[derive(thiserror::Error)]
enum InjectStatementError {
    #[error("Not allowed")]
    NotAllowed,
    #[error("Invalid proof")]
    InvalidProofs,
    #[error("Node access error: {0}")]
    NodeAccess(#[from] QueryError),
    #[error("Error acquiring internal lock.")]
    LockingError,
    #[error("Proof provided for an unknown session.")]
    UnknownSession,
}

impl From<RPCError> for InjectStatementError {
    fn from(err: RPCError) -> Self { Self::NodeAccess(err.into()) }
}

impl warp::reject::Reject for InjectStatementError {}

#[derive(serde::Serialize)]
/// Response in case of an error. This is going to be encoded as a JSON body
/// with fields 'code' and 'message'.
struct ErrorResponse {
    code:    u16,
    message: String,
}

/// Helper function to make the reply.
fn mk_reply(message: String, code: StatusCode) -> impl warp::Reply {
    let msg = ErrorResponse {
        message,
        code: code.as_u16(),
    };
    warp::reply::with_status(warp::reply::json(&msg), code)
}

async fn handle_rejection(err: Rejection) -> Result<impl warp::Reply, Infallible> {
    if err.is_not_found() {
        let code = StatusCode::NOT_FOUND;
        let message = "Not found.";
        Ok(mk_reply(message.into(), code))
    } else if let Some(InjectStatementError::NotAllowed) = err.find() {
        let code = StatusCode::BAD_REQUEST;
        let message = "Needs proof.";
        Ok(mk_reply(message.into(), code))
    } else if let Some(InjectStatementError::InvalidProofs) = err.find() {
        let code = StatusCode::BAD_REQUEST;
        let message = "Invalid proofs.";
        Ok(mk_reply(message.into(), code))
    } else if let Some(InjectStatementError::NodeAccess(e)) = err.find() {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = format!("Cannot access the node: {}", e);
        Ok(mk_reply(message, code))
    } else if let Some(InjectStatementError::LockingError) = err.find() {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = format!("Could not acquire lock.");
        Ok(mk_reply(message, code))
    } else if let Some(InjectStatementError::UnknownSession) = err.find() {
        let code = StatusCode::NOT_FOUND;
        let message = format!("Session not found.");
        Ok(mk_reply(message, code))
    } else if err
        .find::<warp::filters::body::BodyDeserializeError>()
        .is_some()
    {
        let code = StatusCode::BAD_REQUEST;
        let message = "Malformed body.";
        Ok(mk_reply(message.into(), code))
    } else {
        let code = StatusCode::INTERNAL_SERVER_ERROR;
        let message = "Internal error.";
        Ok(mk_reply(message.into(), code))
    }
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
struct ChallengeResponse {
    challenge: Challenge,
    statement: StatementWithContext<ArCurve, AttributeKind>,
}

/// A common function that produces a challange and adds the statement to
/// the state.
async fn inject_statement_worker(
    state: Server,
    request: StatementWithContext<ArCurve, AttributeKind>,
) -> Result<ChallengeResponse, InjectStatementError> {
    let mut challenge = [0u8; 32];
    rand::thread_rng().fill(&mut challenge[..]);
    let mut sm = state
        .statement_map
        .lock()
        .map_err(|_| InjectStatementError::LockingError)?;
    sm.insert(challenge, request.clone());

    Ok(ChallengeResponse {
        challenge,
        statement: request,
    })
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct ChallengedProof {
    challenge: Challenge,
    proof:     Proof<ArCurve, AttributeKind>,
}

/// A common function that validates the cryptographic proofs in the request.
async fn check_proof_worker(
    mut client: concordium_rust_sdk::v2::Client,
    state: Server,
    request: ChallengedProof,
) -> Result<bool, InjectStatementError> {
    let statement = state
        .statement_map
        .lock()
        .map_err(|_| InjectStatementError::LockingError)?
        .get(&request.challenge)
        .ok_or(InjectStatementError::UnknownSession)?
        .clone();
    let cred_id = CredentialRegistrationID::new(statement.credential);
    let acc_info = client
        .get_account_info(&cred_id.into(), BlockIdentifier::LastFinal)
        .await?;
    let credential = acc_info
        .response
        .account_credentials
        .get(&0.into())
        .ok_or(InjectStatementError::InvalidProofs)?;
    let commitments = match &credential.value {
        AccountCredentialWithoutProofs::Initial { icdv: _, .. } => {
            return Err(InjectStatementError::NotAllowed);
        }
        AccountCredentialWithoutProofs::Normal {
            commitments,
            cdv: _,
        } => commitments,
    };

    if statement.verify(
        &request.challenge,
        &state.global_context,
        commitments,
        &request.proof,
    ) {
        Err(InjectStatementError::InvalidProofs)
    } else {
        Ok(true)
    }
}

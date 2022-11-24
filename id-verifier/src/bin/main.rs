use concordium_rust_sdk::{
    endpoints::{QueryError, RPCError},
    id::{
        constants::{ArCurve, AttributeKind},
        id_verifier::verify_attribute_range,
        range_proof::RangeProof,
        types::{AccountAddress, AccountCredentialWithoutProofs, AttributeTag, GlobalContext}, id_proof_types::{StatementWithContext, Proof},
    },
    types::hashes::TransactionHash,
};

use concordium_base::base::CredentialRegistrationID;

use log::{error, info, warn};
use std::{
    collections::{BTreeSet, HashMap},
    convert::Infallible,
    sync::{Arc, Mutex},
};
use structopt::StructOpt;

use warp::{http::StatusCode, Filter, Rejection, Reply};
use rand::Rng;

/// Structure used to receive the correct command line arguments.
#[derive(Debug, StructOpt)]
struct IdVerifierConfig {
    #[structopt(
        long = "node",
        help = "GRPC interface of the node.",
        default_value = "http://localhost:10000"
    )]
    endpoint: concordium_rust_sdk::endpoints::Endpoint,
    #[structopt(
        long = "port",
        default_value = "8100",
        help = "Port on which the server will listen on."
    )]
    port: u16,
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
struct AgeProofOutput {
    account: AccountAddress,
    lower: AttributeKind,
    upper: AttributeKind,
    proof: RangeProof<ArCurve>,
}

// #[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
// #[serde(transparent)]
// struct Basket {
//     basket: Vec<Item>,
// }

type Challenge = [u8; 32];

struct Server {
    statement_map: HashMap<Challenge, StatementWithContext<ArCurve, AttributeKind>>,
    global_context: GlobalContext<ArCurve>,
    // accounts:       BTreeSet<AccountAddress>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let app = IdVerifierConfig::clap()
        .setting(clap::AppSettings::ArgRequiredElseHelp)
        .global_setting(clap::AppSettings::ColoredHelp);
    let matches = app.get_matches();
    let app: IdVerifierConfig = IdVerifierConfig::from_clap(&matches);
    let mut client =
        concordium_rust_sdk::endpoints::Client::connect(app.endpoint, "rpcadmin").await?;
    let consensus_info = client.get_consensus_status().await?;
    let global_context = client
        .get_cryptographic_parameters(&consensus_info.last_finalized_block)
        .await?;
    let state = Arc::new(Mutex::new(Server {
        statement_map: HashMap::new(),
        global_context,
        // accounts: BTreeSet::new(),
    }));
    let add_state = state.clone();
    let prove_state = state.clone();
    let proof_client = client.clone();

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

    info!("Booting up HTTP server. Listening on port {}.", app.port);
    let cors = warp::cors()
        .allow_any_origin()
        .allow_header("Content-Type")
        .allow_method("POST");
    let server = inject_statement
        .or(provide_proof)
        .recover(handle_rejection)
        .with(cors);
    warp::serve(server).run(([0, 0, 0, 0], app.port)).await;
    Ok(())
}

fn handle_inject_statement(
    // client: concordium_rust_sdk::endpoints::Client,
    state: Arc<Mutex<Server>>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::body::json().and_then(move |request: StatementWithContext<ArCurve, AttributeKind>| {
        // let client = client.clone();
        let state = Arc::clone(&state);
        async move {
            info!("Queried for injecting statement");
            match inject_statement_worker(state, request).await {
                Ok(r) => Ok(warp::reply::json(&r)),
                Err(e) => {
                    warn!("Request is invalid {:#?}.", e);
                    Err(warp::reject::custom(e))
                }
            }
        }
    })
}

fn handle_provide_proof(
    client: concordium_rust_sdk::endpoints::Client,
    state: Arc<Mutex<Server>>,
) -> impl Filter<Extract = (impl Reply,), Error = Rejection> + Clone {
    warp::body::json().and_then(move |request: ChallengedProof| {
        let client = client.clone();
        let state = Arc::clone(&state);
        async move {
            info!("Queried for injecting statement");
            match check_proof_worker(client,state, request).await {
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
}


impl From<RPCError> for InjectStatementError {
    fn from(err: RPCError) -> Self {
        Self::NodeAccess(err.into())
    }
}

impl warp::reject::Reject for InjectStatementError {}


#[derive(serde::Serialize)]
/// Response in case of an error. This is going to be encoded as a JSON body
/// with fields 'code' and 'message'.
struct ErrorResponse {
    code: u16,
    message: String,
}

/// Helper function to make the reply.
fn mk_reply(message: String, code: StatusCode) -> impl warp::Reply {
    let msg = ErrorResponse {
        message: message.into(),
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
    state: Arc<Mutex<Server>>,
    request: StatementWithContext<ArCurve, AttributeKind>,
) -> Result<ChallengeResponse, InjectStatementError> {
    let mut challenge = [0u8; 32];
    rand::thread_rng().fill(&mut challenge[..]);
    let mut server = state.lock().expect("Failed to lock");
    server.statement_map.insert(challenge, request.clone());

    Ok(ChallengeResponse{
        challenge,
        statement: request
    })
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
struct ChallengedProof {
    challenge: Challenge,
    proof: Proof<ArCurve, AttributeKind>,
}

/// A common function that validates the cryptographic proofs in the request.
async fn check_proof_worker(
    mut client: concordium_rust_sdk::endpoints::Client,
    state: Arc<Mutex<Server>>,
    request: ChallengedProof,
) -> Result<bool, InjectStatementError> {
    let (statement, global) = {
        let server = state.lock().expect("Failed to lock.");
        (server.statement_map.get(&request.challenge).unwrap().clone(), server.global_context.clone())
    };
    let cred_id = CredentialRegistrationID::new(statement.credential);
    let consensus_info = client.get_consensus_status().await?;
    let acc_info = client
    .get_account_info_by_cred_id(&cred_id, &consensus_info.last_finalized_block)
    .await?;
    let credential = acc_info
        .account_credentials
        .get(&0.into())
        .expect("No credential on account with given index"); // Read the relevant credential from chain that the claim is about.
    let commitments = match &credential.value {
        AccountCredentialWithoutProofs::Initial { icdv: _, .. } => {
            return Err(InjectStatementError::NotAllowed);
        }
        AccountCredentialWithoutProofs::Normal {
            commitments,
            cdv: _,
        } => commitments,
    };

    if statement.verify(&request.challenge, &global, commitments, &request.proof) {
        Err(InjectStatementError::InvalidProofs)
    } else {
        Ok(true)
    }
}

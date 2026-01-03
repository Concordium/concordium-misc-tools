use crate::integration_test_helpers::rest_client::RestClient;
use crate::integration_test_helpers::{fixtures, node_client_mock, rest_client};

use credential_verification_service::configs::ServiceConfigs;
use credential_verification_service::{logging, service};
use std::net::{SocketAddr, TcpStream};

use crate::integration_test_helpers::node_client_mock::NodeClientMock;
use concordium_rust_sdk::id::constants::ArCurve;
use concordium_rust_sdk::id::types::GlobalContext;
use credential_verification_service::node_client::NodeClient;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use std::{thread, time::Duration};
use tracing::info;
use tracing_subscriber::filter;

fn config() -> ServiceConfigs {
    ServiceConfigs {
        node_endpoint: "http://test".parse().unwrap(),
        request_timeout: 5000,
        grpc_node_request_timeout: 1000,
        acquire_account_sequence_lock_timeout: 1000,
        api_address: SocketAddr::new("0.0.0.0".parse().unwrap(), REST_PORT),
        monitoring_address: SocketAddr::new("0.0.0.0".parse().unwrap(), MONITORING_PORT),
        account: "tests/dummyaccount.json".into(),
        log_level: filter::LevelFilter::INFO,
        transaction_expiry_secs: 10,
    }
}

const REST_PORT: u16 = 19000;
const MONITORING_PORT: u16 = 19003;

#[derive(Debug, Clone)]
pub struct ServerHandle {
    shared: ServerHandleShared,
    rest_client: RestClient,
    monitoring_client: RestClient,
}

#[derive(Debug, Clone)]
pub struct ServerHandleShared {
    properties: Arc<ServerProperties>,
    node_client_stub: NodeClientMock,
    global_context: GlobalContext<ArCurve>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ServerProperties {
    pub rest_url: String,
    pub monitoring_url: String,
}

#[allow(dead_code)]
impl ServerHandle {
    pub fn rest_client(&self) -> &RestClient {
        &self.rest_client
    }

    pub fn monitoring_client(&self) -> &RestClient {
        &self.monitoring_client
    }

    pub fn properties(&self) -> &ServerProperties {
        &self.shared.properties
    }

    pub fn global_context(&self) -> &GlobalContext<ArCurve> {
        &self.shared.global_context
    }

    pub fn node_client_stub(&self) -> &NodeClientMock {
        &self.shared.node_client_stub
    }
}

static START_SERVER_ONCE: OnceLock<ServerHandleShared> = OnceLock::new();

/// Start verifier service to be used for integration tests. The returned handle contains:
///
/// * a REST client to interact with the server
/// * a node interface stub to "mock" interactions with the node
///
/// Only a single service is started for all tests. Second time the function is called,
/// a handle to the same service is returned.
pub fn start_server() -> ServerHandle {
    let shared = Clone::clone(START_SERVER_ONCE.get_or_init(start_server_impl));

    let rest_client = rest_client::create_client(shared.properties.rest_url.clone());
    let monitoring_client = rest_client::create_client(shared.properties.monitoring_url.clone());

    ServerHandle {
        shared,
        rest_client,
        monitoring_client,
    }
}

fn start_server_impl() -> ServerHandleShared {
    logging::init_logging(filter::LevelFilter::INFO).unwrap();

    // Create runtime that persists between tests
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .worker_threads(2)
        .build()
        .expect("tokio runtime");

    let config = config();

    let properties = ServerProperties {
        rest_url: format!("http://localhost:{}", REST_PORT),
        monitoring_url: format!("http://localhost:{}", MONITORING_PORT),
    };

    let global_context = fixtures::credentials::global_context();
    let node_client_stub = node_client_mock::node_client(global_context.clone());

    // Start runtime and server in new thread
    let node_client_stub_cloned = node_client_stub.clone();
    thread::spawn(move || runtime.block_on(run_server(config, node_client_stub_cloned)));

    // Wait for server to start
    info!("waiting for verifier service to start");
    let start = Instant::now();
    while TcpStream::connect(format!("localhost:{}", MONITORING_PORT)).is_err() {
        if start.elapsed() > Duration::from_secs(10) {
            panic!("server did not start");
        }

        thread::sleep(Duration::from_millis(500));
    }

    info!(
        "verifier service started with properties:\n{:#?}",
        properties
    );

    ServerHandleShared {
        properties: Arc::new(properties),
        node_client_stub,
        global_context,
    }
}

async fn run_server(config: ServiceConfigs, node_client_stub: NodeClientMock) {
    info!("starting server for test");

    service::run_with_dependencies(config, node_client_stub.boxed())
        .await
        .expect("running server");
}

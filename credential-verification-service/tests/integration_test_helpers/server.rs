use crate::integration_test_helpers::rest_client::RestClient;
use crate::integration_test_helpers::{fixtures, node_client_stub, rest_client};

use credential_verification_service::configs::ServiceConfigs;
use credential_verification_service::{logging, service};
use std::net::{SocketAddr, TcpStream};

use crate::integration_test_helpers::node_client_stub::NodeClientStub;
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
    properties: Arc<ServerProperties>,
    rest_client: RestClient,
    monitoring_client: RestClient,
    node_client_stub: NodeClientStub,
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
        &self.properties
    }

    pub fn global_context(&self) -> &GlobalContext<ArCurve> {
        &self.global_context
    }

    pub fn node_client_stub(&self) -> &NodeClientStub {
        &self.node_client_stub
    }
}

static START_SERVER_ONCE: OnceLock<ServerHandle> = OnceLock::new();

pub fn start_server() -> ServerHandle {
    Clone::clone(START_SERVER_ONCE.get_or_init(|| start_server_impl()))
}

fn start_server_impl() -> ServerHandle {
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
    let node_client_stub = node_client_stub::node_client(global_context.clone());

    // Start runtime and server in new thread
    let node_client_stub_cloned = node_client_stub.clone();
    thread::spawn(move || runtime.block_on(run_server(config, node_client_stub_cloned)));

    // Wait for server to start
    info!("waiting for verifier service to start");
    let start = Instant::now();
    while TcpStream::connect(&format!("localhost:{}", MONITORING_PORT)).is_err() {
        if start.elapsed() > Duration::from_secs(10) {
            panic!("server did not start");
        }

        thread::sleep(Duration::from_millis(500));
    }

    let rest_client = rest_client::create_client(properties.rest_url.clone());
    let monitoring_client = rest_client::create_client(properties.monitoring_url.clone());

    info!(
        "verifier service started with properties:\n{:#?}",
        properties
    );

    ServerHandle {
        properties: Arc::new(properties),
        node_client_stub,
        global_context,
        rest_client,
        monitoring_client,
    }
}

async fn run_server(config: ServiceConfigs, node_client_stub: NodeClientStub) {
    info!("starting server for test");

    service::run_with_dependencies(config, node_client_stub.boxed())
        .await
        .expect("running server")
}

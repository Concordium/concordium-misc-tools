use crate::integration_test_helpers::node_mock::NodeMock;
use crate::integration_test_helpers::rest_client::RestClient;
use crate::integration_test_helpers::{node_mock, rest_client};
use std::net::{SocketAddr, TcpStream};
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use std::{thread, time::Duration};
use tracing::info;
use tracing_subscriber::filter;
use credential_verification_service::configs::ServiceConfigs;
use credential_verification_service::{logging, service};

fn config(node_base_url: &str) -> ServiceConfigs {
    ServiceConfigs {
        node_endpoint: node_base_url.parse().unwrap(),
        request_timeout: 5000,
        grpc_node_request_timeout: 1000,
        api_address: SocketAddr::new("0.0.0.0".parse().unwrap(), REST_PORT),
        monitoring_address: SocketAddr::new("0.0.0.0".parse().unwrap(), MONITORING_PORT),
        account: "tests/dummyaccount.json".into(),
        log_level: filter::LevelFilter::INFO,
        transaction_expiry_secs: 10,
    }
}

struct Stubs {
    config: ServiceConfigs,
}

fn init_stubs(node_base_url: &str) -> Stubs {
    let config = config(node_base_url);

    Stubs { config }
}

const REST_PORT: u16 = 19000;
const MONITORING_PORT: u16 = 19003;

#[derive(Debug, Clone)]
pub struct ServerHandle {
    properties: Arc<ServerProperties>,
    node_mock: NodeMock,
    rest_client: RestClient,
    monitoring_client: RestClient,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ServerProperties {
    pub rest_url: String,
    pub monitoring_url: String,
    pub node_url: String,
}

#[allow(dead_code)]
impl ServerHandle {
    pub fn node_mock(&self) -> &NodeMock {
        &self.node_mock
    }

    pub fn rest_client(&self) -> &RestClient {
        &self.rest_client
    }

    pub fn monitoring_client(&self) -> &RestClient {
        &self.monitoring_client
    }

    pub fn properties(&self) -> &ServerProperties {
        &self.properties
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

    let server_init = ServerStartup::new();
    let rt_handle = runtime.handle().clone();
    let node_mock = thread::spawn(move || rt_handle.block_on(node_mock::init_mock(&server_init)))
        .join()
        .unwrap();

    let stubs = init_stubs(&node_mock.base_url());

    let properties = ServerProperties {
        rest_url: format!("http://localhost:{}", REST_PORT),
        monitoring_url: format!("http://localhost:{}", MONITORING_PORT),
        node_url: stubs.config.node_endpoint.uri().to_string(),
    };

    // Start runtime and server in new thread
    thread::spawn(move || runtime.block_on(run_server(stubs)));

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

    info!("verifier service started with properties:\n{:#?}", properties);

    ServerHandle {
        properties: Arc::new(properties),
        node_mock,
        rest_client,
        monitoring_client,
    }
}

async fn run_server(stubs: Stubs) {
    info!("starting server for test");
    service::run(stubs.config)
        .await
        .expect("running server")
}

pub struct ServerStartup {
    _private: (),
}

impl ServerStartup {
    fn new() -> Self {
        Self { _private: () }
    }
}

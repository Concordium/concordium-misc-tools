use crate::integration_test_helpers::node_stub::NodeStub;
use crate::integration_test_helpers::rest_client::RestClient;
use crate::integration_test_helpers::{fixtures, node_stub, rest_client};

use concordium_rust_sdk::constants;

use concordium_rust_sdk::types::{AbsoluteBlockHeight, GenesisIndex, Nonce, WalletAccount};
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::v2::generated::Empty;
use credential_verification_service::configs::ServiceConfigs;
use credential_verification_service::{logging, service};
use std::net::{SocketAddr, TcpStream};

use std::sync::{Arc, OnceLock};
use std::time::Instant;
use std::{thread, time::Duration};
use tracing::info;
use tracing_subscriber::filter;

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

const REST_PORT: u16 = 19000;
const MONITORING_PORT: u16 = 19003;

#[derive(Debug, Clone)]
pub struct ServerHandle {
    properties: Arc<ServerProperties>,
    node_mock: NodeStub,
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
    pub fn node_stub(&self) -> &NodeStub {
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
    let node_stub = thread::spawn(move || rt_handle.block_on(node_stub::init_stub(&server_init)))
        .join()
        .unwrap();

    let config = config(&node_stub.base_url());

    let properties = ServerProperties {
        rest_url: format!("http://localhost:{}", REST_PORT),
        monitoring_url: format!("http://localhost:{}", MONITORING_PORT),
        node_url: config.node_endpoint.uri().to_string(),
    };

    // Stub node calls done during service start
    let account: WalletAccount = WalletAccount::from_json_file(config.account.clone()).unwrap();
    node_stub.mock(|when, then| {
        when.path("/concordium.v2.Queries/GetNextAccountSequenceNumber")
            .pb(generated::AccountAddress::from(&account.address));
        then.pb(generated::NextAccountSequenceNumber {
            sequence_number: Some(generated::SequenceNumber::from(Nonce::from(1))),
            all_final: true,
        });
    });
    node_stub.mock(|when, then| {
        when.path("/concordium.v2.Queries/GetConsensusInfo")
            .pb(Empty {});
        then.pb(fixtures::chain::consensus_info());
    });

    // Start runtime and server in new thread
    thread::spawn(move || runtime.block_on(run_server(config)));

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
        node_mock: node_stub,
        rest_client,
        monitoring_client,
    }
}

async fn run_server(config: ServiceConfigs) {
    info!("starting server for test");

    service::run(config).await.expect("running server")
}

pub struct ServerStartup {
    _private: (),
}

impl ServerStartup {
    fn new() -> Self {
        Self { _private: () }
    }
}

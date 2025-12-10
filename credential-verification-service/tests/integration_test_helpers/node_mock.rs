use crate::integration_test_helpers::server::ServerStartup;
use mocktail::server::MockServer;
use parking_lot::Mutex;
use std::fmt::{Debug, Formatter};

use mocktail::mock_builder::{Then, When};
use std::sync::Arc;
use tracing::info;

pub async fn init_mock(_startup: &ServerStartup) -> NodeMock {
    let server = MockServer::new_grpc("node");
    server.start().await.unwrap();

    info!("started node mock: {}", server.base_url().unwrap());

    let node_mock = NodeMock {
        server: Arc::new(Mutex::new(server)),
    };

    node_mock
}

#[derive(Clone)]
pub struct NodeMock {
    server: Arc<Mutex<MockServer>>,
}

impl Debug for NodeMock {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeMock")
            .field("server", &"<MockServer>")
            .finish()
    }
}

impl NodeMock {
    pub fn base_url(&self) -> String {
        self.server.lock().base_url().unwrap().as_str().to_owned()
    }

    pub fn mock<F>(&self, f: F)
    where
        F: FnOnce(When, Then),
    {
        self.server.lock().mock(f)
    }
}

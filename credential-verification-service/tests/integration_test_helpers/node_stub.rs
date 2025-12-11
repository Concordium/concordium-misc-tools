use crate::integration_test_helpers::server::ServerStartup;
use mocktail::server::MockServer;
use parking_lot::Mutex;
use std::fmt::{Debug, Formatter};

use mocktail::mock_builder::{Then, When};
use std::sync::Arc;
use tracing::info;

pub async fn init_stub(_startup: &ServerStartup) -> NodeStub {
    let server = MockServer::new_grpc("node");
    server.start().await.unwrap();

    info!("started node mock: {}", server.base_url().unwrap());

    let node_stub = NodeStub {
        server: Arc::new(Mutex::new(server)),
    };

    node_stub
}

#[derive(Clone)]
pub struct NodeStub {
    server: Arc<Mutex<MockServer>>,
}

impl Debug for NodeStub {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NodeMock")
            .field("server", &"<MockServer>")
            .finish()
    }
}

impl NodeStub {
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

/// Test fixtures used to set up test input
pub mod fixtures;
/// Stub implementation of `NodeClient` trait used in integration tests.
pub mod node_client_stub;
/// HTTP client to call verifier server
pub mod rest_client;
/// Logic to start the verifier server
pub mod server;

use concordium_rust_sdk::types::{Nonce, WalletAccount};
use std::sync::Arc;
use concordium_rust_sdk::web3id::did::Network;
use tokio::sync::Mutex;
use crate::node_client::NodeClient;

/// Holds the service state in memory.
///
/// Note: A new instance of this struct is created whenever the service restarts.
#[derive(Debug, Clone)]
pub struct TransactionSubmitter {
    /// The client to interact with the node.
    pub node_client: Box<dyn NodeClient>,
    /// Account that signs transactions
    pub account: Arc<Mutex<AccountWithSequence>>,
    /// The number of seconds in the future when the anchor transactions should expiry.
    pub transaction_expiry_secs: u32,
}

/// Account keys and local view of what the next account sequence number is
#[derive(Debug)]
pub struct AccountWithSequence {
    /// The key and address of the account submitting the anchor transactions on-chain.
    pub account_keys: WalletAccount,
    /// The current nonce of the account submitting the anchor transactions on-chain.
    pub nonce: Nonce,
}

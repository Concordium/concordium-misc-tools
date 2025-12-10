use concordium_rust_sdk::base::hashes::TransactionHash;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

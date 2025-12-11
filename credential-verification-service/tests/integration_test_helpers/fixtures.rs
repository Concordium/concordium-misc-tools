use std::collections::HashMap;
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::common::cbor;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn public_info() -> HashMap<String, cbor::value::Value> {
    [(
        "key1".to_string(),
        cbor::value::Value::Text("value1".to_string()),
    )]
        .into_iter()
        .collect()
}
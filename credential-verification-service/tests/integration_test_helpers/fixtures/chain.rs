use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::constants;
use concordium_rust_sdk::types::{RegisteredData, TransactionStatus};

pub const GENESIS_BLOCK_HASH: [u8; 32] = constants::TESTNET_GENESIS_BLOCK_HASH;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn transaction_status_finalized(
    txn_hash: TransactionHash,
    data: RegisteredData,
) -> TransactionStatus {
    todo!()
}

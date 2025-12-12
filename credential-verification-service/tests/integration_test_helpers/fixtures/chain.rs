use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::constants;
use concordium_rust_sdk::types::{
    AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary,
    BlockItemSummaryDetails, RegisteredData, TransactionIndex, TransactionStatus,
};
use concordium_rust_sdk::v2::Upward;

pub const GENESIS_BLOCK_HASH: [u8; 32] = constants::TESTNET_GENESIS_BLOCK_HASH;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn transaction_status_finalized(
    txn_hash: TransactionHash,
    data: RegisteredData,
) -> TransactionStatus {
    TransactionStatus::Finalized(
        [(
            GENESIS_BLOCK_HASH.into(),
            BlockItemSummary {
                index: TransactionIndex { index: 1 },
                energy_cost: 10.into(),
                hash: txn_hash,
                details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(
                    AccountTransactionDetails {
                        cost: "10".parse().unwrap(),
                        sender: AccountAddress([0u8; 32]),
                        effects: Upward::Known(AccountTransactionEffects::DataRegistered { data }),
                    },
                )),
            },
        )]
        .into_iter()
        .collect(),
    )
}

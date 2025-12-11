use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::types::{ArInfo, GlobalContext, IpInfo};
use concordium_rust_sdk::types::{AbsoluteBlockHeight, BlockHeight, GenesisIndex, RegisteredData};
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::{base, constants};

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn consensus_info() -> generated::ConsensusInfo {
    let block_height = AbsoluteBlockHeight::from(1).into();
    let block_hash = generated::BlockHash {
        value: constants::TESTNET_GENESIS_BLOCK_HASH.into(),
    };

    generated::ConsensusInfo {
        last_finalized_block_height: Some(block_height),
        block_arrive_latency_emsd: 0.0,
        block_receive_latency_emsd: 0.0,
        last_finalized_block: Some(block_hash.clone()),
        block_receive_period_emsd: Some(0.0),
        block_arrive_period_emsd: Some(0.0),
        blocks_received_count: 1,
        transactions_per_block_emsd: 0.0,
        finalization_period_ema: Some(0.0),
        best_block_height: Some(block_height),
        last_finalized_time: Some(generated::Timestamp { value: 1 }),
        finalization_count: 1,
        epoch_duration: Some(generated::Duration { value: 1 }),
        blocks_verified_count: 1,
        slot_duration: Some(generated::Duration { value: 1 }),
        genesis_time: Some(generated::Timestamp { value: 1 }),
        finalization_period_emsd: Some(0.0),
        transactions_per_block_ema: 0.0,
        block_arrive_latency_ema: 0.0,
        block_receive_latency_ema: 0.0,
        block_arrive_period_ema: Some(0.0),
        block_receive_period_ema: Some(0.0),
        block_last_arrived_time: Some(generated::Timestamp { value: 1 }),
        best_block: Some(block_hash.clone()),
        genesis_block: Some(block_hash.clone()),
        block_last_received_time: Some(generated::Timestamp { value: 1 }),
        protocol_version: 1,
        genesis_index: Some(GenesisIndex::from(1).into()),
        current_era_genesis_block: Some(block_hash),
        current_era_genesis_time: Some(generated::Timestamp { value: 1 }),
        current_timeout_duration: Some(generated::Duration { value: 1 }),
        current_round: Some(generated::Round { value: 1 }),
        current_epoch: Some(generated::Epoch { value: 1 }),
        trigger_block_time: Some(generated::Timestamp { value: 1 }),
    }
}

pub fn block_info() -> generated::BlockInfo {
    let abs_block_height = AbsoluteBlockHeight::from(1).into();
    let block_height = BlockHeight::from(1).into();
    let block_hash = generated::BlockHash {
        value: constants::TESTNET_GENESIS_BLOCK_HASH.into(),
    };

    generated::BlockInfo {
        hash: Some(block_hash.clone()),
        height: Some(abs_block_height),
        parent_block: Some(block_hash.clone()),
        last_finalized_block: Some(block_hash),
        genesis_index: Some(GenesisIndex::from(1).into()),
        era_block_height: Some(block_height),
        receive_time: Some(generated::Timestamp { value: 1 }),
        arrive_time: Some(generated::Timestamp { value: 1 }),
        slot_number: Some(generated::Slot { value: 1 }),
        slot_time: Some(generated::Timestamp { value: 1 }),
        baker: Some(generated::BakerId { value: 1 }),
        finalized: false,
        transaction_count: 1,
        transactions_energy_cost: Some(generated::Energy { value: 10 }),
        transactions_size: 1,
        state_hash: Some(generated::StateHash {
            value: [1u8; 32].into(),
        }),
        protocol_version: 1,
        round: Some(generated::Round { value: 1 }),
        epoch: Some(generated::Epoch { value: 1 }),
    }
}

pub fn data_registration_block_item_finalized(
    txn_hash: TransactionHash,
    data: RegisteredData,
) -> generated::BlockItemStatus {
    generated::BlockItemStatus {
        status: Some(generated::block_item_status::Status::Finalized(
            generated::block_item_status::Finalized {
                outcome: Some(generated::BlockItemSummaryInBlock {
                    block_hash: Some(generated::BlockHash {
                        value: constants::TESTNET_GENESIS_BLOCK_HASH.into(),
                    }),
                    outcome: Some(generated::BlockItemSummary {
                        index: Some(generated::block_item_summary::TransactionIndex { value: 1 }),
                        energy_cost: Some(generated::Energy { value: 10 }),
                        hash: Some((&txn_hash).into()),
                        details: Some(generated::block_item_summary::Details::AccountTransaction(
                            generated::AccountTransactionDetails {
                                cost: Some(generated::Amount{ value: 20 }),
                                sender: Some(generated::AccountAddress { value: [2u8;32].into() }),
                                effects: Some(generated::AccountTransactionEffects {
                                    effect: Some(generated::account_transaction_effects::Effect::DataRegistered(generated::RegisteredData { value: data.into() })),
                                }),
                            },
                        )),
                    }),
                }),
            },
        )),
    }
}

pub fn map_ip_info(ip_info: &IpInfo<IpPairing>) -> generated::IpInfo {
    generated::IpInfo {
        identity: Some(generated::IpIdentity {
            value: ip_info.ip_identity.0,
        }),
        description: Some(generated::Description {
            name: ip_info.ip_description.name.clone(),
            url: ip_info.ip_description.url.clone(),
            description: ip_info.ip_description.description.clone(),
        }),
        verify_key: Some(generated::ip_info::IpVerifyKey {
            value: base::common::to_bytes(&ip_info.ip_verify_key),
        }),
        cdi_verify_key: Some(generated::ip_info::IpCdiVerifyKey {
            value: base::common::to_bytes(&ip_info.ip_cdi_verify_key),
        }),
    }
}

pub fn map_ar_info(ar_info: &ArInfo<ArCurve>) -> generated::ArInfo {
    generated::ArInfo {
        identity: Some(generated::ar_info::ArIdentity {
            value: ar_info.ar_identity.into(),
        }),
        description: Some(generated::Description {
            name: ar_info.ar_description.name.clone(),
            url: ar_info.ar_description.url.clone(),
            description: ar_info.ar_description.description.clone(),
        }),
        public_key: Some(generated::ar_info::ArPublicKey {
            value: base::common::to_bytes(&ar_info.ar_public_key),
        }),
    }
}

pub fn cryptographic_parameters(global_context: &GlobalContext<ArCurve>) -> generated::CryptographicParameters {
    generated::CryptographicParameters {
        genesis_string: "test".to_string(),
        bulletproof_generators: base::common::to_bytes(&global_context.bulletproof_generators),
        on_chain_commitment_key: base::common::to_bytes(& global_context.on_chain_commitment_key),
    }
}
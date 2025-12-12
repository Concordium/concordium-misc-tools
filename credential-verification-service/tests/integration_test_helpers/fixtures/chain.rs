use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::pedersen_commitment::Commitment;
use concordium_rust_sdk::id::constants::{ArCurve, IpPairing};
use concordium_rust_sdk::id::types::{ArInfo, AttributeTag, GlobalContext, IpIdentity, IpInfo};
use concordium_rust_sdk::types::{AbsoluteBlockHeight, BlockHeight, GenesisIndex, RegisteredData};
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::{base, constants};
use std::collections::BTreeMap;

pub const BLOCK_HASH: [u8; 32] = constants::TESTNET_GENESIS_BLOCK_HASH;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn consensus_info() -> generated::ConsensusInfo {
    let block_height = AbsoluteBlockHeight::from(1).into();
    let block_hash = generated::BlockHash {
        value: BLOCK_HASH.into(),
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
        value: BLOCK_HASH.into(),
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
                        value: BLOCK_HASH.into(),
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

pub fn cryptographic_parameters(
    global_context: &GlobalContext<ArCurve>,
) -> generated::CryptographicParameters {
    generated::CryptographicParameters {
        genesis_string: "test".to_string(),
        bulletproof_generators: base::common::to_bytes(&global_context.bulletproof_generators),
        on_chain_commitment_key: base::common::to_bytes(&global_context.on_chain_commitment_key),
    }
}


pub fn account_info(
    issuer: &IpIdentity,
    commitments: &BTreeMap<AttributeTag, Commitment<ArCurve>>,
) -> generated::AccountInfo {
    generated::AccountInfo {
        sequence_number: Some(Default::default()),
        amount: Some(Default::default()),
        schedule: Some(generated::ReleaseSchedule {
            total: Some(Default::default()),
            schedules: vec![],
        }),
        creds: [(
            0,
            generated::AccountCredential {
                credential_values: Some(generated::account_credential::CredentialValues::Normal({
                    generated::NormalCredentialValues {
                        keys: Some(generated::CredentialPublicKeys {
                            keys: [].into_iter().collect(),
                            threshold: Some(generated::SignatureThreshold {
                                value: 1,
                            }),
                        }),
                        cred_id: Some(generated::CredentialRegistrationId {
                            value: hex::decode("a075536bd5aa8cae5067ca084b787d0f2b50af6f40a9c661585880c7917132c15bf4f326848b7b577aa118c32f8da129").unwrap(),
                        }),
                        ip_id: Some(generated::IdentityProviderIdentity {
                            value: issuer.0,
                        }),
                        policy: Some(generated::Policy {
                            created_at: Some(Default::default()),
                            valid_to: Some(Default::default()),
                            attributes: Default::default(),
                        }),
                        ar_threshold: Some(generated::ArThreshold {
                            value: 1,
                        }),
                        ar_data: Default::default(),
                        commitments: Some(generated::CredentialCommitments {
                            prf: Some(generated::Commitment {
                                value: vec![0u8;48],
                            }),
                            cred_counter: Some(generated::Commitment {
                                value: vec![0u8;48],
                            }),
                            max_accounts: Some(generated::Commitment {
                                value: vec![0u8;48],
                            }),
                            attributes: Default::default(),
                            id_cred_sec_sharing_coeff: vec![],
                        }),

                    }
                })),
            },
        )].into_iter().collect(),
        threshold: Some(generated::AccountThreshold {
            value: 1,
        }),
        encrypted_balance: Some(generated::EncryptedBalance {
            self_amount: Some(generated::EncryptedAmount {
                value: hex::decode("c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap(),
            }),
            start_index: 0,
            aggregated_amount: None,
            num_aggregated: None,
            incoming_amounts: vec![],
        }),
        encryption_key: Some(generated::EncryptionKey {
            value: hex::decode("b14cbfe44a02c6b1f78711176d5f437295367aa4f2a8c2551ee10d25a03adc69d61a332a058971919dad7312e1fc94c5a075536bd5aa8cae5067ca084b787d0f2b50af6f40a9c661585880c7917132c15bf4f326848b7b577aa118c32f8da129").unwrap(),
        }),
        index: Some(Default::default()),
        stake: None,
        address: Some(generated::AccountAddress {
            value: vec![0u8;32],
        }),
        cooldowns: vec![],
        available_balance: Some(Default::default()),
        tokens: vec![],
    }
}

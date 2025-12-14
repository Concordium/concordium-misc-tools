use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::base::curve_arithmetic::Curve;
use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::pedersen_commitment::Commitment;
use concordium_rust_sdk::common::Versioned;
use concordium_rust_sdk::constants;
use concordium_rust_sdk::id::constants::ArCurve;
use concordium_rust_sdk::id::secret_sharing::Threshold;
use concordium_rust_sdk::id::types::{
    AccountCredentialWithoutProofs, AttributeTag, CredentialDeploymentCommitments,
    CredentialDeploymentValues, CredentialPublicKeys, IpIdentity, Policy, YearMonth,
};
use concordium_rust_sdk::types::{
    AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary,
    BlockItemSummaryDetails, CredentialRegistrationID, RegisteredData, TransactionIndex,
    TransactionStatus,
};
use concordium_rust_sdk::v2::Upward;
use credential_verification_service::node_client::AccountCredentials;
use std::collections::BTreeMap;

pub const GENESIS_BLOCK_HASH: [u8; 32] = constants::TESTNET_GENESIS_BLOCK_HASH;

pub fn account_address(id: u8) -> AccountAddress {
    AccountAddress([id; 32])
}

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
                        sender: account_address(10),
                        effects: Upward::Known(AccountTransactionEffects::DataRegistered { data }),
                    },
                )),
            },
        )]
        .into_iter()
        .collect(),
    )
}

pub fn account_credentials(
    cred_id: &CredentialRegistrationID,
    ip_identity: IpIdentity,
    cmm_attributes: BTreeMap<AttributeTag, Commitment<ArCurve>>,
) -> AccountCredentials {
    let cred = AccountCredentialWithoutProofs::Normal {
        cdv: CredentialDeploymentValues {
            cred_key_info: CredentialPublicKeys {
                keys: Default::default(),
                threshold: 1.try_into().unwrap(),
            },
            cred_id: *cred_id.as_ref(),
            ip_identity,
            threshold: Threshold(1),
            ar_data: Default::default(),
            policy: Policy {
                created_at: YearMonth::new(2020, 5).unwrap(),
                policy_vec: Default::default(),
                valid_to: YearMonth::new(2050, 5).unwrap(),
                _phantom: Default::default(),
            },
        },
        commitments: CredentialDeploymentCommitments {
            cmm_prf: Commitment(ArCurve::zero_point()),
            cmm_cred_counter: Commitment(ArCurve::zero_point()),
            cmm_max_accounts: Commitment(ArCurve::zero_point()),
            cmm_attributes,
            cmm_id_cred_sec_sharing_coeff: vec![Commitment(ArCurve::zero_point())],
        },
    };

    let cred = Versioned::new(0.into(), Upward::Known(cred));
    [(1.into(), cred)].into_iter().collect()
}

//! Input configuration structures and parsing.
//!
//! All TOML deserialization types live here. The SDK (`concordium_rust_sdk::genesis`)
//! is free of TOML / serde-Deserialize concerns; this crate owns all TOML parsing
//! and converts to the library's typed values via `TryFrom` / `Into`.

use concordium_rust_sdk::{
    common::{types::Amount, SerdeDeserialize},
    id,
    id::types::{ArIdentity, IpIdentity, SignatureThreshold},
    types::{UpdateKeysIndex, UpdateKeysThreshold},
};
use std::path::PathBuf;

/// Struct for specifying the cryptographic parameters. Either a path to a file
/// with existing gryptographic parameters, or the genesis string from which
/// theg5 cryptographic parameters should be generated.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CryptoParamsConfig {
    #[serde(rename_all = "camelCase")]
    Existing { source: PathBuf },
    #[serde(rename_all = "camelCase")]
    Generate { genesis_string: String },
}

/// Struct for specifying one or more genesis anonymity revokers. Either a path
/// to a file with an existing anonymity revoker, or an id for which an
/// anonymity revoker should be generated freshly. If the `repeat` is `Some(n)`,
/// it specifies that `n` anonymity revokers should be generated freshly,
/// starting from the given id.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AnonymityRevokerConfig {
    #[serde(rename_all = "camelCase")]
    Existing { source: PathBuf },
    #[serde(rename_all = "camelCase")]
    Fresh { id: ArIdentity, repeat: Option<u32> },
}

/// Struct for specifying one or more genesis identity providers. Either a path
/// to a file with an existing identity provider, or an id for which an identity
/// provider should be generated freshly. If the `repeat` is `Some(n)`, it
/// specifies that `n` identity providers should be generated freshly, starting
/// from the given id.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum IdentityProviderConfig {
    #[serde(rename_all = "camelCase")]
    Existing { source: PathBuf },
    #[serde(rename_all = "camelCase")]
    Fresh {
        id: id::types::IpIdentity,
        repeat: Option<u32>,
    },
}

/// Struct for specifying one or more genesis accounts. Either a path to a file
/// with an existing account is given, or a fresh account should be generated
/// freshly.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AccountConfig {
    #[serde(rename_all = "camelCase")]
    Existing {
        source: PathBuf,
        balance: Amount,
        stake: Option<Amount>,
        #[serde(default)]
        restake_earnings: bool,
        baker_keys: Option<PathBuf>,
        #[serde(default)]
        foundation: bool,
    },
    #[serde(rename_all = "camelCase")]
    Fresh {
        // if repeat, the first account gets used as a foundation account
        repeat: Option<u32>,
        stake: Option<Amount>,
        balance: Amount,
        template: String,
        identity_provider: IpIdentity,
        // default to 1
        num_keys: Option<u8>,
        // default to 1
        threshold: Option<SignatureThreshold>,
        #[serde(default)]
        restake_earnings: bool,
        #[serde(default)]
        foundation: bool,
    },
}

/// Struct for specifying which level 2 keys can authorize a concrete level 2
/// chain update, together with a threshold specifying how many of the given
/// keys are needed.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Level2UpdateConfig {
    pub authorized_keys: Vec<UpdateKeysIndex>,
    pub threshold: UpdateKeysThreshold,
}

/// Struct holding all the level 2 keys and for each level 2 chain update the
/// keys `Level2UpdateConfig` determining the keys that can authorize the
/// update.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Level2KeysConfig {
    pub keys: Vec<HigherLevelKey>,
    pub emergency: Level2UpdateConfig,
    pub protocol: Level2UpdateConfig,
    pub election_difficulty: Level2UpdateConfig,
    pub euro_per_energy: Level2UpdateConfig,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro: Level2UpdateConfig,
    pub foundation_account: Level2UpdateConfig,
    pub mint_distribution: Level2UpdateConfig,
    pub transaction_fee_distribution: Level2UpdateConfig,
    pub gas_rewards: Level2UpdateConfig,
    pub pool_parameters: Level2UpdateConfig,
    pub add_anonymity_revoker: Level2UpdateConfig,
    pub add_identity_provider: Level2UpdateConfig,
    // Optional because it is not needed in P1-P3,
    pub cooldown_parameters: Option<Level2UpdateConfig>,
    pub time_parameters: Option<Level2UpdateConfig>,
    pub create_plt: Option<Level2UpdateConfig>,
}

/// Struct holding the root or the level 1 keys, together with a threshold.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HigherLevelKeysConfig {
    pub threshold: UpdateKeysThreshold,
    pub keys: Vec<HigherLevelKey>,
}

/// Struct for specifying a key. Either a path to an existing key, or a `u32`
/// specifying how many keys should be generated freshly.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HigherLevelKey {
    Existing { source: PathBuf },
    Fresh { repeat: u32 },
}

/// Struct holding all the root, level 1 and level 2 keys.
#[derive(SerdeDeserialize, Debug)]
pub struct UpdateKeysConfig {
    pub root: HigherLevelKeysConfig,
    pub level1: HigherLevelKeysConfig,
    pub level2: Level2KeysConfig,
}

/// For specifying where to ouput chain update keys, account keys, baker keys,
/// identity providers, anonymity revokers, cryptographic parameters and the
/// `genesis.dat` file. The `delete_existing` field specifies whether to delete
/// existing files before generation.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OutputConfig {
    pub update_keys: Option<PathBuf>,
    pub account_keys: PathBuf,
    pub baker_keys: PathBuf,
    pub identity_providers: PathBuf,
    pub anonymity_revokers: PathBuf,
    pub genesis: PathBuf,
    pub genesis_hash: PathBuf,
    pub cryptographic_parameters: Option<PathBuf>,
    #[serde(default)]
    pub delete_existing: bool,
}

/// Struct representing the configuration specified by the input TOML file for
/// every protocol version.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// Configuration of the output files.
    pub out: OutputConfig,
    /// Configuration of the keys for chain updates. This includes parameter
    /// updates, and authorization updates.
    pub updates: UpdateKeysConfig,
    /// Configuration for generating cryptographic parameters for the chain.
    pub cryptographic_parameters: CryptoParamsConfig,
    /// Configuration for generating anonymity revokers.
    pub anonymity_revokers: Vec<AnonymityRevokerConfig>,
    /// Configuration for generating identity providers.
    pub identity_providers: Vec<IdentityProviderConfig>,
    /// Configuration for generating accounts.
    pub accounts: Vec<AccountConfig>,
    /// Protocol specific configurations.
    #[serde(flatten)]
    pub protocol: ProtocolConfigToml,
}

// ── Shared TOML deserialization helpers (CPV2 + CPV3) ────────────────────────

use concordium_rust_sdk::genesis::CoreGenesisParametersV1;
use concordium_rust_sdk::{
    common::types::Ratio,
    types::{FinalizationCommitteeParameters, PartsPerHundredThousands},
};

/// TOML-deserializable finalization committee parameters.
///
/// Converts to the SDK's [`FinalizationCommitteeParameters`] via [`TryFrom`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FinalizationCommitteeParametersConfig {
    pub min_finalizers: u32,
    pub max_finalizers: u32,
    pub finalizers_relative_stake_threshold: u32,
}

impl TryFrom<FinalizationCommitteeParametersConfig> for FinalizationCommitteeParameters {
    type Error = anyhow::Error;

    fn try_from(config: FinalizationCommitteeParametersConfig) -> anyhow::Result<Self> {
        Ok(Self {
            min_finalizers: config.min_finalizers,
            max_finalizers: config.max_finalizers,
            finalizers_relative_stake_threshold: PartsPerHundredThousands::new(
                config.finalizers_relative_stake_threshold,
            )
            .ok_or_else(|| anyhow::anyhow!("finalizers_relative_stake_threshold exceeds 100000"))?,
        })
    }
}

/// Helper for deserializing a [`Ratio`] from a numerator/denominator pair.
#[derive(Debug, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatioNumDenomSerde {
    pub numerator: u64,
    pub denominator: u64,
}

impl TryFrom<RatioNumDenomSerde> for Ratio {
    type Error = concordium_rust_sdk::common::types::NewRatioError;

    fn try_from(ratio: RatioNumDenomSerde) -> Result<Ratio, Self::Error> {
        Ratio::new(ratio.numerator, ratio.denominator)
    }
}

/// TOML-deserializable core genesis parameters for CPV1/CPV2 protocols (P4+).
///
/// Converts to the library's [`CoreGenesisParametersV1`] via [`TryFrom`].
#[derive(Debug, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreGenesisParametersConfigV1 {
    pub genesis_time: Option<chrono::DateTime<chrono::Utc>>,
    pub epoch_duration: concordium_rust_sdk::smart_contracts::common::Duration,
    pub signature_threshold: RatioNumDenomSerde,
}

impl TryFrom<CoreGenesisParametersConfigV1> for CoreGenesisParametersV1 {
    type Error = anyhow::Error;

    fn try_from(config: CoreGenesisParametersConfigV1) -> anyhow::Result<Self> {
        let time = if let Some(date) = config.genesis_time {
            date.timestamp_millis()
        } else {
            chrono::Utc::now().timestamp_millis()
        };
        anyhow::ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        let genesis_time = concordium_rust_sdk::common::types::Timestamp {
            millis: time as u64,
        };
        let threshold = Ratio::try_from(config.signature_threshold)?;
        let threshold_decimal = rust_decimal::Decimal::from(threshold);
        let min_threshold = rust_decimal::Decimal::from(2) / rust_decimal::Decimal::from(3);
        anyhow::ensure!(
            min_threshold <= threshold_decimal,
            "Signature threshold must be 2/3 or larger."
        );
        anyhow::ensure!(
            threshold.numerator() <= threshold.denominator(),
            "Signature threshold must be 1 or less."
        );
        Ok(CoreGenesisParametersV1 {
            genesis_time,
            epoch_duration: config.epoch_duration,
            signature_threshold: threshold,
        })
    }
}

// ── TOML types for CPV3 (P8+) ─────────────────────────────────────────
//
// These types own the TOML deserialization for chain parameters version 3.
// The library's `GenesisChainParametersV3` is the typed "pending" representation
// without serde; these wrappers parse the TOML and convert to it.

use concordium_rust_sdk::genesis::{
    GenesisChainParametersV3, ProtocolParamsCPV3, RewardParametersCPV2,
};
use concordium_rust_sdk::{
    smart_contracts::common::Duration,
    types::{
        CooldownParameters, Energy, ExchangeRate, GASRewardsV1, MintDistributionV1, PoolParameters,
        TimeParameters, TimeoutParameters, ValidatorScoreParameters,
    },
};

/// TOML-deserializable reward parameters for CPV2 (P6+).
///
/// Converts to the SDK's [`RewardParametersCPV2`] via [`From`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RewardParamsCPV2Toml {
    pub mint_distribution: MintDistributionV1,
    pub transaction_fee_distribution: concordium_rust_sdk::types::TransactionFeeDistribution,
    #[serde(rename = "gASRewards")]
    pub gas_rewards: GASRewardsV1,
}

impl From<RewardParamsCPV2Toml> for RewardParametersCPV2 {
    fn from(t: RewardParamsCPV2Toml) -> Self {
        RewardParametersCPV2 {
            mint_distribution: t.mint_distribution,
            transaction_fee_distribution: t.transaction_fee_distribution,
            gas_rewards: t.gas_rewards,
        }
    }
}

/// TOML-deserializable CPV3 chain parameters.
///
/// Converts to the library's [`GenesisChainParametersV3`] via [`TryFrom`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV3Toml {
    pub timeout_parameters: TimeoutParameters,
    pub min_block_time: Duration,
    pub block_energy_limit: Energy,
    pub euro_per_energy: ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro: ExchangeRate,
    pub account_creation_limit: u16,
    pub reward_parameters: RewardParamsCPV2Toml,
    pub time_parameters: TimeParameters,
    pub pool_parameters: PoolParameters,
    pub cooldown_parameters: CooldownParameters,
    pub finalization_committee_parameters: FinalizationCommitteeParametersConfig,
    pub validator_score_parameters: ValidatorScoreParameters,
}

impl TryFrom<GenesisChainParametersV3Toml> for GenesisChainParametersV3 {
    type Error = anyhow::Error;

    fn try_from(t: GenesisChainParametersV3Toml) -> anyhow::Result<Self> {
        Ok(GenesisChainParametersV3 {
            timeout_parameters: t.timeout_parameters,
            min_block_time: t.min_block_time,
            block_energy_limit: t.block_energy_limit,
            euro_per_energy: t.euro_per_energy,
            micro_ccd_per_euro: t.micro_ccd_per_euro,
            account_creation_limit: t.account_creation_limit.into(),
            reward_parameters: t.reward_parameters.into(),
            time_parameters: t.time_parameters,
            pool_parameters: t.pool_parameters,
            cooldown_parameters: t.cooldown_parameters,
            finalization_committee_parameters: t.finalization_committee_parameters.try_into()?,
            validator_score_parameters: t.validator_score_parameters,
        })
    }
}

/// TOML-deserializable CPV3 genesis parameters (P8+).
///
/// Converts to the library's [`ProtocolParamsCPV3`] via [`TryFrom`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigV2Cli {
    pub leadership_election_nonce: concordium_rust_sdk::types::hashes::LeadershipElectionNonce,
    #[serde(flatten)]
    pub core: CoreGenesisParametersConfigV1,
    pub chain: GenesisChainParametersV3Toml,
}

impl TryFrom<GenesisParametersConfigV2Cli> for ProtocolParamsCPV3 {
    type Error = anyhow::Error;

    fn try_from(cfg: GenesisParametersConfigV2Cli) -> anyhow::Result<Self> {
        Ok(ProtocolParamsCPV3 {
            core: cfg.core.try_into()?,
            chain: cfg.chain.try_into()?,
            leadership_election_nonce: cfg.leadership_election_nonce,
        })
    }
}

/// Full TOML-deserializable protocol configuration.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "protocolVersion")]
pub enum ProtocolConfigToml {
    #[serde(rename = "1")]
    P1 {
        parameters: GenesisParametersConfigV0,
    },
    #[serde(rename = "2")]
    P2 {
        parameters: GenesisParametersConfigV0,
    },
    #[serde(rename = "3")]
    P3 {
        parameters: GenesisParametersConfigV0,
    },
    #[serde(rename = "4")]
    P4 {
        parameters: GenesisParametersConfigCpV1Cli,
    },
    #[serde(rename = "5")]
    P5 {
        parameters: GenesisParametersConfigCpV1Cli,
    },
    #[serde(rename = "6")]
    P6 {
        parameters: GenesisParametersConfigV1Cli,
    },
    #[serde(rename = "7")]
    P7 {
        parameters: GenesisParametersConfigV1Cli,
    },
    #[serde(rename = "8")]
    P8 {
        parameters: GenesisParametersConfigV2Cli,
    },
    #[serde(rename = "9")]
    P9 {
        parameters: GenesisParametersConfigV2Cli,
    },
    #[serde(rename = "10")]
    P10 {
        parameters: GenesisParametersConfigV2Cli,
    },
    #[serde(rename = "11")]
    P11 {
        parameters: GenesisParametersConfigV2Cli,
    },
}

impl ProtocolConfigToml {
    pub fn protocol_version(&self) -> concordium_rust_sdk::types::ProtocolVersion {
        use concordium_rust_sdk::types::ProtocolVersion;
        match self {
            Self::P1 { .. } => ProtocolVersion::P1,
            Self::P2 { .. } => ProtocolVersion::P2,
            Self::P3 { .. } => ProtocolVersion::P3,
            Self::P4 { .. } => ProtocolVersion::P4,
            Self::P5 { .. } => ProtocolVersion::P5,
            Self::P6 { .. } => ProtocolVersion::P6,
            Self::P7 { .. } => ProtocolVersion::P7,
            Self::P8 { .. } => ProtocolVersion::P8,
            Self::P9 { .. } => ProtocolVersion::P9,
            Self::P10 { .. } => ProtocolVersion::P10,
            Self::P11 { .. } => ProtocolVersion::P11,
        }
    }
}

// ── TOML types for CPV2 (P6–P7) ─────────────────────────────────────────

use concordium_rust_sdk::genesis::{GenesisChainParametersV2, ProtocolParamsCPV2};

/// TOML-deserializable CPV2 chain parameters.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV2Toml {
    pub timeout_parameters: concordium_rust_sdk::types::TimeoutParameters,
    pub min_block_time: concordium_rust_sdk::smart_contracts::common::Duration,
    pub block_energy_limit: concordium_rust_sdk::types::Energy,
    pub euro_per_energy: concordium_rust_sdk::types::ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro: concordium_rust_sdk::types::ExchangeRate,
    pub account_creation_limit: u16,
    pub reward_parameters: RewardParamsCPV2Toml,
    pub time_parameters: concordium_rust_sdk::types::TimeParameters,
    pub pool_parameters: concordium_rust_sdk::types::PoolParameters,
    pub cooldown_parameters: concordium_rust_sdk::types::CooldownParameters,
    pub finalization_committee_parameters: FinalizationCommitteeParametersConfig,
}

impl TryFrom<GenesisChainParametersV2Toml> for GenesisChainParametersV2 {
    type Error = anyhow::Error;
    fn try_from(t: GenesisChainParametersV2Toml) -> anyhow::Result<Self> {
        Ok(GenesisChainParametersV2 {
            timeout_parameters: t.timeout_parameters,
            min_block_time: t.min_block_time,
            block_energy_limit: t.block_energy_limit,
            euro_per_energy: t.euro_per_energy,
            micro_ccd_per_euro: t.micro_ccd_per_euro,
            account_creation_limit: t.account_creation_limit.into(),
            reward_parameters: t.reward_parameters.into(),
            time_parameters: t.time_parameters,
            pool_parameters: t.pool_parameters,
            cooldown_parameters: t.cooldown_parameters,
            finalization_committee_parameters: t.finalization_committee_parameters.try_into()?,
        })
    }
}

/// TOML-deserializable CPV2 genesis parameters (P6–P7).
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigV1Cli {
    pub leadership_election_nonce: concordium_rust_sdk::types::hashes::LeadershipElectionNonce,
    #[serde(flatten)]
    pub core: CoreGenesisParametersConfigV1,
    pub chain: GenesisChainParametersV2Toml,
}

impl TryFrom<GenesisParametersConfigV1Cli> for ProtocolParamsCPV2 {
    type Error = anyhow::Error;
    fn try_from(cfg: GenesisParametersConfigV1Cli) -> anyhow::Result<Self> {
        Ok(ProtocolParamsCPV2 {
            core: cfg.core.try_into()?,
            chain: cfg.chain.try_into()?,
            leadership_election_nonce: cfg.leadership_election_nonce,
        })
    }
}

// ── TOML types for CPV1 (P4–P5) ─────────────────────────────────────────

use concordium_rust_sdk::genesis::{
    GenesisChainParametersV1, ProtocolParamsCPV1, RewardParametersCPV1,
};
use concordium_rust_sdk::types::GASRewards as GASRewardsCPV1;

/// TOML-deserializable reward parameters for CPV1 (P4–P5).
///
/// Converts to the SDK's [`RewardParametersCPV1`] via [`From`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RewardParamsCPV1Toml {
    pub mint_distribution: MintDistributionV1,
    pub transaction_fee_distribution: concordium_rust_sdk::types::TransactionFeeDistribution,
    #[serde(rename = "gASRewards")]
    pub gas_rewards: GASRewardsCPV1,
}

impl From<RewardParamsCPV1Toml> for RewardParametersCPV1 {
    fn from(t: RewardParamsCPV1Toml) -> Self {
        RewardParametersCPV1 {
            mint_distribution: t.mint_distribution,
            transaction_fee_distribution: t.transaction_fee_distribution,
            gas_rewards: t.gas_rewards,
        }
    }
}

/// TOML-deserializable CPV1 chain parameters.
/// P4 and P5 always use `version = "v1"` chain parameters.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV1Toml {
    pub election_difficulty: concordium_rust_sdk::types::ElectionDifficulty,
    pub euro_per_energy: concordium_rust_sdk::types::ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro: concordium_rust_sdk::types::ExchangeRate,
    pub account_creation_limit: u16,
    pub reward_parameters: RewardParamsCPV1Toml,
    pub time_parameters: concordium_rust_sdk::types::TimeParameters,
    pub pool_parameters: concordium_rust_sdk::types::PoolParameters,
    pub cooldown_parameters: concordium_rust_sdk::types::CooldownParameters,
}

impl From<GenesisChainParametersV1Toml> for GenesisChainParametersV1 {
    fn from(t: GenesisChainParametersV1Toml) -> Self {
        GenesisChainParametersV1 {
            election_difficulty: t.election_difficulty,
            euro_per_energy: t.euro_per_energy,
            micro_ccd_per_euro: t.micro_ccd_per_euro,
            account_creation_limit: t.account_creation_limit.into(),
            reward_parameters: t.reward_parameters.into(),
            time_parameters: t.time_parameters,
            pool_parameters: t.pool_parameters,
            cooldown_parameters: t.cooldown_parameters,
        }
    }
}

/// Wrapper for P4/P5 chain params — v1 only (P4 always uses CPV1).
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "version")]
pub enum GenesisChainParamsV1Only {
    #[serde(rename = "v1")]
    V1(GenesisChainParametersV1Toml),
}

/// TOML-deserializable P4/P5 genesis parameters.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigCpV1Cli {
    pub genesis_time: Option<chrono::DateTime<chrono::Utc>>,
    pub slot_duration: concordium_rust_sdk::types::SlotDuration,
    pub leadership_election_nonce: concordium_rust_sdk::types::hashes::LeadershipElectionNonce,
    pub epoch_length: u64,
    pub finalization: concordium_rust_sdk::genesis::FinalizationParameters,
    pub max_block_energy: concordium_rust_sdk::types::Energy,
    pub chain: GenesisChainParamsV1Only,
}

impl TryFrom<GenesisParametersConfigCpV1Cli> for ProtocolParamsCPV1 {
    type Error = anyhow::Error;
    fn try_from(cfg: GenesisParametersConfigCpV1Cli) -> anyhow::Result<Self> {
        let time = cfg.genesis_time.map_or_else(
            || chrono::Utc::now().timestamp_millis(),
            |x| x.timestamp_millis(),
        );
        anyhow::ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        let core = concordium_rust_sdk::genesis::CoreGenesisParametersV0 {
            time: concordium_rust_sdk::common::types::Timestamp {
                millis: time as u64,
            },
            slot_duration: cfg.slot_duration,
            epoch_length: cfg.epoch_length,
            max_block_energy: cfg.max_block_energy,
            finalization_parameters: cfg.finalization,
        };
        let GenesisChainParamsV1Only::V1(chain_toml) = cfg.chain;
        Ok(ProtocolParamsCPV1 {
            core,
            chain: chain_toml.into(),
            leadership_election_nonce: cfg.leadership_election_nonce,
        })
    }
}

// ── TOML types for CPV0 (P1–P3) ─────────────────────────────────────────

use concordium_rust_sdk::genesis::{
    GenesisChainParametersV0, ProtocolParamsCPV0, RewardParametersCPV0,
};
use concordium_rust_sdk::types::{GASRewards, MintDistributionV0, TransactionFeeDistribution};
use concordium_rust_sdk::{common::types::Amount as MicroCCDAmount, types::Epoch};

/// TOML-deserializable reward parameters for CPV0 (P1–P3).
///
/// Converts to the SDK's [`RewardParametersCPV0`] via [`From`].
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct RewardParamsCPV0Toml {
    pub mint_distribution: MintDistributionV0,
    pub transaction_fee_distribution: TransactionFeeDistribution,
    #[serde(rename = "gASRewards")]
    pub gas_rewards: GASRewards,
}

impl From<RewardParamsCPV0Toml> for RewardParametersCPV0 {
    fn from(t: RewardParamsCPV0Toml) -> Self {
        RewardParametersCPV0 {
            mint_distribution: t.mint_distribution,
            transaction_fee_distribution: t.transaction_fee_distribution,
            gas_rewards: t.gas_rewards,
        }
    }
}

/// TOML-deserializable CPV0 chain parameters.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV0Toml {
    pub election_difficulty: concordium_rust_sdk::types::ElectionDifficulty,
    pub euro_per_energy: concordium_rust_sdk::types::ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro: concordium_rust_sdk::types::ExchangeRate,
    pub account_creation_limit: u16,
    pub baker_cooldown_epochs: Epoch,
    pub reward_parameters: RewardParamsCPV0Toml,
    pub minimum_threshold_for_baking: MicroCCDAmount,
}

impl From<GenesisChainParametersV0Toml> for GenesisChainParametersV0 {
    fn from(t: GenesisChainParametersV0Toml) -> Self {
        GenesisChainParametersV0 {
            election_difficulty: t.election_difficulty,
            euro_per_energy: t.euro_per_energy,
            micro_ccd_per_euro: t.micro_ccd_per_euro,
            account_creation_limit: t.account_creation_limit.into(),
            baker_cooldown_epochs: t.baker_cooldown_epochs,
            reward_parameters: t.reward_parameters.into(),
            minimum_threshold_for_baking: t.minimum_threshold_for_baking,
        }
    }
}

/// TOML-deserializable CPV0 chain parameter wrapper (always v0).
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "version")]
pub enum GenesisChainParametersToml {
    #[serde(rename = "v0")]
    V0(GenesisChainParametersV0Toml),
}

/// GenesisParametersConfigV0 (moved from library).
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigV0 {
    pub genesis_time: Option<chrono::DateTime<chrono::Utc>>,
    pub slot_duration: concordium_rust_sdk::types::SlotDuration,
    pub leadership_election_nonce: concordium_rust_sdk::types::hashes::LeadershipElectionNonce,
    pub epoch_length: u64,
    pub finalization: concordium_rust_sdk::genesis::FinalizationParameters,
    pub max_block_energy: concordium_rust_sdk::types::Energy,
    pub chain: GenesisChainParametersToml,
}

impl TryFrom<GenesisParametersConfigV0> for ProtocolParamsCPV0 {
    type Error = anyhow::Error;
    fn try_from(cfg: GenesisParametersConfigV0) -> anyhow::Result<Self> {
        let time = cfg.genesis_time.map_or_else(
            || chrono::Utc::now().timestamp_millis(),
            |x| x.timestamp_millis(),
        );
        anyhow::ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        let core = concordium_rust_sdk::genesis::CoreGenesisParametersV0 {
            time: concordium_rust_sdk::common::types::Timestamp {
                millis: time as u64,
            },
            slot_duration: cfg.slot_duration,
            epoch_length: cfg.epoch_length,
            max_block_energy: cfg.max_block_energy,
            finalization_parameters: cfg.finalization,
        };
        let GenesisChainParametersToml::V0(chain_toml) = cfg.chain;
        Ok(ProtocolParamsCPV0 {
            core,
            chain: chain_toml.into(),
            leadership_election_nonce: cfg.leadership_election_nonce,
        })
    }
}

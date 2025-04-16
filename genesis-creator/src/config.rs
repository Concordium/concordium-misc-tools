//! Input configuration structures and parsing.
use crate::{
    genesis::{GenesisParametersConfigV0, GenesisParametersConfigV1},
    GenesisParametersConfigV2,
};
use anyhow::ensure;

use concordium_rust_sdk::{
    common::{types::Amount, SerdeDeserialize},
    id,
    id::types::{ArIdentity, IpIdentity, SignatureThreshold},
    types::{
        AccessStructure, ProtocolVersion, UpdateKeysIndex, UpdateKeysThreshold, UpdatePublicKey,
    },
};
use std::{collections::BTreeSet, path::PathBuf};

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
    Fresh {
        id:     ArIdentity,
        repeat: Option<u32>,
    },
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
        id:     id::types::IpIdentity,
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
        source:           PathBuf,
        balance:          Amount,
        stake:            Option<Amount>,
        #[serde(default)]
        restake_earnings: bool,
        baker_keys:       Option<PathBuf>,
        #[serde(default)]
        foundation:       bool,
    },
    #[serde(rename_all = "camelCase")]
    Fresh {
        // if repeat, the first account gets used as a foundation account
        repeat:            Option<u32>,
        stake:             Option<Amount>,
        balance:           Amount,
        template:          String,
        identity_provider: IpIdentity,
        // default to 1
        num_keys:          Option<u8>,
        // default to 1
        threshold:         Option<SignatureThreshold>,
        #[serde(default)]
        restake_earnings:  bool,
        #[serde(default)]
        foundation:        bool,
    },
}

/// Struct for specifying which level 2 keys can authorize a concrete level 2
/// chain update, together with a threshold specifying how many of the given
/// keys are needed.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Level2UpdateConfig {
    pub authorized_keys: Vec<UpdateKeysIndex>,
    pub threshold:       UpdateKeysThreshold,
}

impl Level2UpdateConfig {
    pub fn access_structure(self, ctx: &[UpdatePublicKey]) -> anyhow::Result<AccessStructure> {
        let num_given_keys = self.authorized_keys.len();
        let authorized_keys: BTreeSet<_> = self.authorized_keys.into_iter().collect();
        ensure!(
            authorized_keys.len() == num_given_keys,
            "Duplicate key index provided."
        );
        for key_idx in authorized_keys.iter() {
            ensure!(
                usize::from(key_idx.index) < ctx.len(),
                "Key index {} does not specify a known update key.",
                key_idx.index
            );
        }
        ensure!(
            usize::from(u16::from(self.threshold)) <= num_given_keys,
            "Threshold exceeds the number of keys."
        );
        Ok(AccessStructure {
            authorized_keys,
            threshold: self.threshold,
        })
    }
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
}

/// Struct holding the root or the level 1 keys, together with a threshold.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct HigherLevelKeysConfig {
    pub threshold: UpdateKeysThreshold,
    pub keys:      Vec<HigherLevelKey>,
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
    pub root:   HigherLevelKeysConfig,
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
    pub update_keys:              Option<PathBuf>,
    pub account_keys:             PathBuf,
    pub baker_keys:               PathBuf,
    pub identity_providers:       PathBuf,
    pub anonymity_revokers:       PathBuf,
    pub genesis:                  PathBuf,
    pub genesis_hash:             PathBuf,
    pub cryptographic_parameters: Option<PathBuf>,
    #[serde(default)]
    pub delete_existing:          bool,
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
    pub protocol: ProtocolConfig,
}

/// Protocol specific configurations, tagged by the protocol version.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "protocolVersion")]
pub enum ProtocolConfig {
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
        parameters: GenesisParametersConfigV0,
    },
    #[serde(rename = "5")]
    P5 {
        parameters: GenesisParametersConfigV0,
    },
    #[serde(rename = "6")]
    P6 {
        parameters: GenesisParametersConfigV1,
    },
    #[serde(rename = "7")]
    P7 {
        parameters: GenesisParametersConfigV1,
    },
    #[serde(rename = "8")]
    P8 {
        parameters: GenesisParametersConfigV2,
    },
    #[serde(rename = "8")]
    P9 {
        parameters: GenesisParametersConfigV2,
    },
}

impl ProtocolConfig {
    pub fn protocol_version(&self) -> ProtocolVersion {
        match self {
            ProtocolConfig::P1 { .. } => ProtocolVersion::P1,
            ProtocolConfig::P2 { .. } => ProtocolVersion::P2,
            ProtocolConfig::P3 { .. } => ProtocolVersion::P3,
            ProtocolConfig::P4 { .. } => ProtocolVersion::P4,
            ProtocolConfig::P5 { .. } => ProtocolVersion::P5,
            ProtocolConfig::P6 { .. } => ProtocolVersion::P6,
            ProtocolConfig::P7 { .. } => ProtocolVersion::P7,
            ProtocolConfig::P8 { .. } => ProtocolVersion::P8,
            ProtocolConfig::P9 { .. } => ProtocolVersion::P9,
        }
    }
}

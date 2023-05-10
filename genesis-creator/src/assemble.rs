//! Configuration specific to the `assemble` functionality.
use concordium_rust_sdk::{common::SerdeDeserialize, id::types::AccountAddress};
use std::path::PathBuf;

use crate::config::ProtocolConfig;

/// Configuration struct for specifying protocol version, the genesis
/// parameters, the foundation account and where to find genesis accounts,
/// anonymity revokers, identity providers, cryptographic parameters and where
/// to output the genesis data file.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AssembleGenesisConfig {
    /// Protocol specific configurations.
    #[serde(flatten)]
    pub protocol:           ProtocolConfig,
    /// Address of the account to set as the initial foundation account.
    pub foundation_account: AccountAddress,
    /// A file with a list of accounts that should be assembled into
    /// genesis.
    pub accounts:           PathBuf,
    /// A file with a list of anonymity revokers that should be assembled
    /// into genesis.
    pub ars:                PathBuf,
    /// A file with a list of identity providers that should be assembled
    /// into genesis.
    pub idps:               PathBuf,
    /// A file pointing to the cryptographic parameters to be put into
    /// genesis.
    pub global:             PathBuf,
    /// A file pointing to the governance (root, level 1, and level 2) keys.
    pub governance_keys:    PathBuf,
    /// Location where to output the genesis block.
    pub genesis_out:        PathBuf,
    /// Location where to output the genesis block hash.
    pub genesis_hash_out:   PathBuf,
}

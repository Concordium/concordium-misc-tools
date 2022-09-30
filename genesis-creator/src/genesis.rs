use anyhow::{ensure, Context};
use concordium_rust_sdk::{
    common::{
        self as crypto_common,
        derive::{Serial, Serialize},
        types::{Amount, CredentialIndex, Timestamp},
        Buffer, Deserial, Get, ParseResult, ReadBytesExt, SerdeDeserialize, SerdeSerialize, Serial,
        Versioned,
    },
    id,
    id::{
        constants::{ArCurve, IpPairing},
        types::{
            AccCredentialInfo, AccountAddress, AccountCredentialWithoutProofs, AccountKeys,
            ArIdentity, ArInfo, GlobalContext, IpIdentity, IpInfo,
        },
    },
    types::{
        hashes::LeadershipElectionNonce, AccountIndex, AccountThreshold, BakerAggregationVerifyKey,
        BakerElectionVerifyKey, BakerId, BakerSignatureVerifyKey, BlockHeight,
        ChainParameterVersion0, ChainParameterVersion1, ChainParameters, ChainParametersV0,
        ChainParametersV1, CooldownParameters, ElectionDifficulty, Energy, Epoch, ExchangeRate,
        PoolParameters, ProtocolVersion, RewardParameters, SlotDuration, TimeParameters,
        UpdateKeysCollection,
    },
};
use gcd::Gcd;
use serde::de::{self};
use std::collections::BTreeMap;

/// A type alias for credentials in a format suitable for genesis. Genesis
/// credentials do not have any associated proofs.
pub type GenesisCredentials = BTreeMap<
    CredentialIndex,
    AccountCredentialWithoutProofs<id::constants::ArCurve, id::constants::AttributeKind>,
>;

/// Private genesis account data. When generating fresh accounts, these are
/// output as JSON files. When using existing accounts, these are instead input
/// as JSON files. The format is the same as what is expected when importing
/// genesis accounts with concordium-client.
#[derive(SerdeDeserialize, SerdeSerialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccount {
    pub account_keys:          AccountKeys,
    pub aci:                   AccCredentialInfo<id::constants::ArCurve>,
    pub address:               AccountAddress,
    pub credentials:           Versioned<GenesisCredentials>,
    pub encryption_public_key: id::elgamal::PublicKey<id::constants::ArCurve>,
    pub encryption_secret_key: id::elgamal::SecretKey<id::constants::ArCurve>,
}

/// Struct corresponding to the Haskell type `GenesisBaker` in
/// `haskell-src/Concordium/Genesis/Account.hs` in `concordium-base`.
#[derive(Serialize, SerdeSerialize, SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisBakerPublic {
    /// Initial stake of the baker.
    pub stake:                  Amount,
    /// Whether earnings should be automatically restaked or not.
    pub restake_earnings:       bool,
    /// The ID of the baker. This must correspond to the account index, which is
    /// the place in the list of genesis accounts.
    pub baker_id:               BakerId,
    pub election_verify_key:    BakerElectionVerifyKey,
    pub signature_verify_key:   BakerSignatureVerifyKey,
    pub aggregation_verify_key: BakerAggregationVerifyKey,
}

/// Struct corresponding to the Haskell type `GenesisAccount` in
/// `haskell-src/Concordium/Genesis/Account.hs` in `concordium-base`.
#[derive(Serialize, SerdeSerialize, SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisAccountPublic {
    pub address:           AccountAddress,
    pub account_threshold: AccountThreshold,
    #[map_size_length = 8]
    #[serde(deserialize_with = "deserialize_versioned_public_account")]
    pub credentials:       GenesisCredentials,
    pub balance:           Amount,
    pub baker:             Option<GenesisBakerPublic>,
}

fn deserialize_versioned_public_account<'de, D: de::Deserializer<'de>>(
    des: D,
) -> Result<GenesisCredentials, D::Error> {
    let versioned: Versioned<GenesisCredentials> =
        Versioned::<GenesisCredentials>::deserialize(des)?;
    Ok(versioned.value)
}

/// A ratio between two `u64` integers.
#[derive(Debug, SerdeDeserialize, Serial, Clone, Copy)]
#[serde(try_from = "rust_decimal::Decimal")]
struct Ratio {
    numerator:   u64,
    denominator: u64,
}

impl Deserial for Ratio {
    fn deserial<R: ReadBytesExt>(source: &mut R) -> ParseResult<Self> {
        let numerator: u64 = source.get()?;
        let denominator = source.get()?;
        ensure!(denominator != 0, "Denominator cannot be 0.");
        ensure!(
            numerator.gcd(denominator) == 1,
            "Numerator and denominator must be coprime."
        );
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

impl TryFrom<rust_decimal::Decimal> for Ratio {
    type Error = anyhow::Error;

    fn try_from(mut value: rust_decimal::Decimal) -> Result<Self, Self::Error> {
        value.normalize_assign();
        let mantissa = value.mantissa();
        let scale = value.scale();
        let denominator = 10u64.checked_pow(scale).context("Unrepresentable number")?;
        let numerator: u64 = mantissa.try_into().context("Unrepresentable number")?;
        let g = numerator.gcd(denominator);
        let numerator = numerator / g;
        let denominator = denominator / g;
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

/// The finalization parameters. Corresponds to the Haskell type
/// `FinalizationParameters` in haskell-src/Concordium/Types/Parameters.hs.
#[derive(SerdeDeserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FinalizationParameters {
    /// Number of levels to skip between finalizations.
    minimum_skip:        BlockHeight,
    /// Maximum size of the finalization committee; determines the minimum stake
    ///  required to join the committee as @totalGTU /
    /// finalizationCommitteeMaxSize@.
    committee_max_size:  u32,
    /// Base delay time used in finalization, in milliseconds.
    waiting_time:        u64,
    /// Factor used to shrink the finalization gap. Must be strictly between 0
    /// and 1.
    skip_shrink_factor:  Ratio,
    /// Factor used to grow the finalization gap. Must be strictly greater than
    /// 1.
    skip_grow_factor:    Ratio,
    /// Factor for shrinking the finalization delay (i.e. number of descendent
    /// blocks required to be eligible as a finalization target).
    delay_shrink_factor: Ratio,
    /// Factor for growing the finalization delay when it takes more than one
    /// round to finalize a block.
    delay_grow_factor:   Ratio,
    /// Whether to allow the delay to be 0. (This allows a block to be finalized
    /// as soon as it is baked.)
    allow_zero_delay:    bool,
}

/// Genesis chain parameters version 0. Contains all version 0 chain paramters
/// except for the foundation index.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV0 {
    election_difficulty:          ElectionDifficulty,
    euro_per_energy:              ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    micro_ccd_per_euro:           ExchangeRate,
    account_creation_limit:       u16,
    baker_cooldown_epochs:        Epoch,
    reward_parameters:            RewardParameters<ChainParameterVersion0>,
    minimum_threshold_for_baking: Amount,
}

impl GenesisChainParametersV0 {
    pub fn chain_parameters(self, foundation_account_index: AccountIndex) -> ChainParametersV0 {
        let Self {
            election_difficulty,
            euro_per_energy,
            micro_ccd_per_euro,
            account_creation_limit,
            baker_cooldown_epochs,
            reward_parameters,
            minimum_threshold_for_baking,
        } = self;
        ChainParametersV0 {
            election_difficulty,
            euro_per_energy,
            micro_gtu_per_euro: micro_ccd_per_euro,
            baker_cooldown_epochs,
            account_creation_limit: account_creation_limit.into(),
            reward_parameters,
            foundation_account_index,
            minimum_threshold_for_baking,
        }
    }
}

/// Genesis chain parameters version 0. Contains all version 1 chain paramters
/// except for the foundation index.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV1 {
    election_difficulty:    ElectionDifficulty,
    euro_per_energy:        ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    micro_ccd_per_euro:     ExchangeRate,
    account_creation_limit: u16,
    reward_parameters:      RewardParameters<ChainParameterVersion1>,
    time_parameters:        TimeParameters,
    pool_parameters:        PoolParameters,
    cooldown_parameters:    CooldownParameters,
}

impl GenesisChainParametersV1 {
    pub fn chain_parameters(self, foundation_account_index: AccountIndex) -> ChainParametersV1 {
        let Self {
            election_difficulty,
            euro_per_energy,
            micro_ccd_per_euro,
            account_creation_limit,
            time_parameters,
            pool_parameters,
            cooldown_parameters,
            reward_parameters,
        } = self;
        ChainParametersV1 {
            election_difficulty,
            euro_per_energy,
            micro_gtu_per_euro: micro_ccd_per_euro,
            time_parameters,
            pool_parameters,
            cooldown_parameters,
            account_creation_limit: account_creation_limit.into(),
            reward_parameters,
            foundation_account_index,
        }
    }
}

/// Genesis chain parameters and the version.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "version")]
pub enum GenesisChainParameters {
    #[serde(rename = "v0")]
    V0(GenesisChainParametersV0),
    #[serde(rename = "v1")]
    V1(GenesisChainParametersV1),
}

/// The core genesis parameters, the leadership election nonce and the chain
/// parameters (except the foundation index).
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParameters {
    // Time at which the genesis will occur.
    pub genesis_time:              chrono::DateTime<chrono::Utc>,
    // Duration of a slot in milliseconds
    pub slot_duration:             SlotDuration,
    // Leadership election nonce.
    pub leadership_election_nonce: LeadershipElectionNonce,
    // Number of slots that go into an epoch.
    pub epoch_length:              u64,
    // Finalization parameters.
    pub finalization:              FinalizationParameters,
    // Max energy that is allowed for a block.
    pub max_block_energy:          Energy,
    pub chain:                     GenesisChainParameters,
}

/// The core genesis parameters. This corresponds to the Haskell type in
/// haskell-src/Concordium/Genesis/Data/Base.hs in concordium-base.
#[derive(Debug, Serialize)]
pub struct CoreGenesisParameters {
    time:                    Timestamp,
    slot_duration:           SlotDuration,
    epoch_length:            u64,
    max_block_energy:        Energy,
    finalization_parameters: FinalizationParameters,
}

impl GenesisParameters {
    pub fn to_core(&self) -> anyhow::Result<CoreGenesisParameters> {
        let time = self.genesis_time.timestamp_millis();
        ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        Ok(CoreGenesisParameters {
            time:                    Timestamp {
                millis: time as u64,
            },
            slot_duration:           self.slot_duration,
            epoch_length:            self.epoch_length,
            max_block_energy:        self.max_block_energy,
            finalization_parameters: self.finalization.clone(),
        })
    }
}

/// The genesis state in chain parameters version 0. This corresponds to the
/// Haskell type `GenesisState` from haskell-src/Concordium/Genesis/Data/Base.hs
/// for those protocol versions having chain parameters version 0.
#[derive(Debug)]
pub struct GenesisStateCPV0 {
    pub cryptographic_parameters:  GlobalContext<ArCurve>,
    pub identity_providers:        BTreeMap<IpIdentity, IpInfo<IpPairing>>,
    pub anonymity_revokers:        BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    pub update_keys:               UpdateKeysCollection<ChainParameterVersion0>,
    pub chain_parameters:          ChainParameters<ChainParameterVersion0>,
    pub leadership_election_nonce: LeadershipElectionNonce,
    pub accounts:                  Vec<GenesisAccountPublic>,
}

fn serialize_with_length_header(data: &impl Serial, buf: &mut Vec<u8>, out: &mut impl Buffer) {
    data.serial(buf);
    (buf.len() as u32).serial(out);
    out.write_all(buf).expect("Writing to buffers succeeds.");
    buf.clear();
}

impl Serial for GenesisStateCPV0 {
    fn serial<B: Buffer>(&self, out: &mut B) {
        let mut tmp = Vec::new();
        serialize_with_length_header(&self.cryptographic_parameters, &mut tmp, out);
        (self.identity_providers.len() as u32).serial(out);
        for (k, v) in self.identity_providers.iter() {
            k.serial(out);
            serialize_with_length_header(v, &mut tmp, out);
        }
        (self.anonymity_revokers.len() as u32).serial(out);
        for (k, v) in self.anonymity_revokers.iter() {
            k.serial(out);
            serialize_with_length_header(v, &mut tmp, out);
        }
        self.update_keys.serial(out);
        self.chain_parameters.serial(out);
        self.leadership_election_nonce.serial(out);
        self.accounts.serial(out)
    }
}

/// The genesis state in chain parameters version 1. This corresponds to the
/// Haskell type `GenesisState` from haskell-src/Concordium/Genesis/Data/Base.hs
/// for those protocol versions having chain parameters version 1, currently
/// only P4.
#[derive(Debug)]
pub struct GenesisStateCPV1 {
    pub cryptographic_parameters:  GlobalContext<ArCurve>,
    pub identity_providers:        BTreeMap<IpIdentity, IpInfo<IpPairing>>,
    pub anonymity_revokers:        BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    pub update_keys:               UpdateKeysCollection<ChainParameterVersion1>,
    pub chain_parameters:          ChainParameters<ChainParameterVersion1>,
    pub leadership_election_nonce: LeadershipElectionNonce,
    pub accounts:                  Vec<GenesisAccountPublic>,
}

impl Serial for GenesisStateCPV1 {
    fn serial<B: Buffer>(&self, out: &mut B) {
        let mut tmp = Vec::new();
        serialize_with_length_header(&self.cryptographic_parameters, &mut tmp, out);
        (self.identity_providers.len() as u32).serial(out);
        for (k, v) in self.identity_providers.iter() {
            k.serial(out);
            serialize_with_length_header(v, &mut tmp, out);
        }
        (self.anonymity_revokers.len() as u32).serial(out);
        for (k, v) in self.anonymity_revokers.iter() {
            k.serial(out);
            serialize_with_length_header(v, &mut tmp, out);
        }
        self.update_keys.serial(out);
        self.chain_parameters.serial(out);
        self.leadership_election_nonce.serial(out);
        self.accounts.serial(out)
    }
}

/// The genesis data containing the core genesis parameters and the initial
/// genesis state.
pub enum GenesisData {
    P1 {
        core:          CoreGenesisParameters,
        initial_state: GenesisStateCPV0,
    },
    P2 {
        core:          CoreGenesisParameters,
        initial_state: GenesisStateCPV0,
    },
    P3 {
        core:          CoreGenesisParameters,
        initial_state: GenesisStateCPV0,
    },
    P4 {
        core:          CoreGenesisParameters,
        initial_state: GenesisStateCPV1,
    },
}

pub fn make_genesis_data_cpv0(
    pv: ProtocolVersion,
    core: CoreGenesisParameters,
    initial_state: GenesisStateCPV0,
) -> Option<GenesisData> {
    match pv {
        ProtocolVersion::P1 => Some(GenesisData::P1 {
            core,
            initial_state,
        }),
        ProtocolVersion::P2 => Some(GenesisData::P2 {
            core,
            initial_state,
        }),
        ProtocolVersion::P3 => Some(GenesisData::P3 {
            core,
            initial_state,
        }),
        ProtocolVersion::P4 => None,
    }
}

impl Serial for GenesisData {
    fn serial<B: Buffer>(&self, out: &mut B) {
        match self {
            GenesisData::P1 {
                core,
                initial_state,
            } => {
                // version of the genesis
                3u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
            GenesisData::P2 {
                core,
                initial_state,
            } => {
                4u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
            GenesisData::P3 {
                core,
                initial_state,
            } => {
                5u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
            GenesisData::P4 {
                core,
                initial_state,
            } => {
                6u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
        }
    }
}

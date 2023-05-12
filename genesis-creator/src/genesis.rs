use anyhow::{ensure, Context};
use concordium_rust_sdk::{
    base as concordium_base,
    common::{
        types::{Amount, CredentialIndex, Timestamp},
        Buffer, Deserial, Get, ParseResult, ReadBytesExt, SerdeDeserialize, SerdeSerialize, Serial,
        Serialize, Versioned,
    },
    id,
    id::{
        constants::{ArCurve, IpPairing},
        types::{
            AccCredentialInfo, AccountAddress, AccountCredentialWithoutProofs, AccountKeys,
            ArIdentity, ArInfo, GlobalContext, IpIdentity, IpInfo,
        },
    },
    smart_contracts::common::Duration,
    types::{
        hashes::{BlockHash, LeadershipElectionNonce},
        AccountIndex, AccountThreshold, BakerAggregationVerifyKey, BakerElectionVerifyKey, BakerId,
        BakerSignatureVerifyKey, BlockHeight, ChainParameterVersion0, ChainParameterVersion1,
        ChainParameterVersion2, ChainParameters, ChainParametersV0, ChainParametersV1,
        ChainParametersV2, CooldownParameters, ElectionDifficulty, Energy, Epoch, ExchangeRate,
        PartsPerHundredThousands, PoolParameters, ProtocolVersion, RewardParameters, Slot,
        SlotDuration, TimeParameters, TimeoutParameters, UpdateKeysCollection,
    },
};
use gcd::Gcd;
use serde::de;
use sha2::Digest;
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
pub struct Ratio {
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

impl From<Ratio> for num::rational::Ratio<u64> {
    fn from(ratio: Ratio) -> Self { Self::new_raw(ratio.numerator, ratio.denominator) }
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
/// except for the foundation account index.
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

/// Genesis chain parameters version 1. Contains all version 1 chain parameters
/// except for the foundation account index.
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

/// Genesis chain parameters version 2.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisChainParametersV2 {
    /// Consensus protocol version 2 timeout parameters.
    pub timeout_parameters:                TimeoutParametersConfig,
    /// Minimum time interval between blocks.
    pub min_block_time:                    SlotDuration,
    /// Maximum energy allowed per block.
    pub block_energy_limit:                Energy,
    pub euro_per_energy:                   ExchangeRate,
    #[serde(rename = "microCCDPerEuro")]
    pub micro_ccd_per_euro:                ExchangeRate,
    pub account_creation_limit:            u16,
    pub reward_parameters:                 RewardParameters<ChainParameterVersion2>,
    pub time_parameters:                   TimeParameters,
    pub pool_parameters:                   PoolParameters,
    pub cooldown_parameters:               CooldownParameters,
    pub finalization_committee_parameters: FinalizationCommitteeParametersConfig,
}

impl GenesisChainParametersV2 {
    pub fn chain_parameters(
        self,
        foundation_account_index: AccountIndex,
    ) -> anyhow::Result<ChainParametersV2> {
        Ok(ChainParametersV2 {
            timeout_parameters: self.timeout_parameters.into(),
            min_block_time: Duration::from_millis(self.min_block_time.millis),
            block_energy_limit: self.block_energy_limit,
            euro_per_energy: self.euro_per_energy,
            micro_ccd_per_euro: self.micro_ccd_per_euro,
            time_parameters: self.time_parameters,
            pool_parameters: self.pool_parameters,
            cooldown_parameters: self.cooldown_parameters,
            account_creation_limit: self.account_creation_limit.into(),
            reward_parameters: self.reward_parameters,
            foundation_account_index,
            finalization_committee_parameters: self.finalization_committee_parameters.try_into()?,
        })
    }
}

/// Parameters controlling consensus timeouts for the consensus protocol version
/// 2.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TimeoutParametersConfig {
    /// The base value for triggering a timeout.
    pub base:     SlotDuration,
    /// Factor for increasing the timeout. Must be greater than 1.
    pub increase: Ratio,
    /// Factor for decreasing the timeout. Must be between 0 and 1.
    pub decrease: Ratio,
}

impl From<TimeoutParametersConfig> for TimeoutParameters {
    fn from(config: TimeoutParametersConfig) -> Self {
        Self {
            base:     Duration::from_millis(config.base.millis),
            increase: config.increase.into(),
            decrease: config.decrease.into(),
        }
    }
}

#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FinalizationCommitteeParametersConfig {
    /// Minimum number of bakers to include in the finalization committee before
    /// the '_fcpFinalizerRelativeStakeThreshold' takes effect.
    pub min_finalizers: u32,
    /// Maximum number of bakers to include in the finalization committee.
    pub max_finalizers: u32,
    /// Determining the staking threshold required for being eligible the
    /// finalization committee.
    pub finalizers_relative_stake_threshold: u32,
}

impl TryFrom<FinalizationCommitteeParametersConfig>
    for concordium_rust_sdk::types::FinalizationCommitteeParameters
{
    type Error = anyhow::Error;

    fn try_from(config: FinalizationCommitteeParametersConfig) -> Result<Self, Self::Error> {
        Ok(Self {
            min_finalizers: config.min_finalizers,
            max_finalizers: config.max_finalizers,
            finalizers_relative_stake_threshold: PartsPerHundredThousands::new(
                config.finalizers_relative_stake_threshold,
            )
            .context("Part exceeds 100000")?,
        })
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

impl GenesisChainParameters {
    pub fn election_difficulty(&self) -> ElectionDifficulty {
        match self {
            GenesisChainParameters::V0(cp) => cp.election_difficulty,
            GenesisChainParameters::V1(cp) => cp.election_difficulty,
        }
    }
}

/// The core genesis parameters, the leadership election nonce and the chain
/// parameters (except the foundation account index).
///
/// Used to derive parsing for the genesis parameter section of the TOML config.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigV0 {
    /// Time at which the genesis will occur. If `None` then the tool will use
    /// "current" time as genesis time.
    pub genesis_time:              Option<chrono::DateTime<chrono::Utc>>,
    /// Duration of a slot in milliseconds
    pub slot_duration:             SlotDuration,
    /// Leadership election nonce.
    pub leadership_election_nonce: LeadershipElectionNonce,
    /// Number of slots that go into an epoch.
    pub epoch_length:              u64,
    /// Finalization parameters.
    pub finalization:              FinalizationParameters,
    /// Max energy that is allowed for a block.
    pub max_block_energy:          Energy,
    pub chain:                     GenesisChainParameters,
}

impl GenesisParametersConfigV0 {
    /// Convert genesis parameters to [`CoreGenesisParameters`]. Note that this
    /// function is effectful in that, if the genesis time is not provided it
    /// will use the current time as genesis time.
    pub fn to_core(&self) -> anyhow::Result<CoreGenesisParametersV0> {
        let time = self.genesis_time.map_or_else(
            || chrono::Utc::now().timestamp_millis(),
            |x| x.timestamp_millis(),
        );
        ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        Ok(CoreGenesisParametersV0 {
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

/// The core genesis parameters. This corresponds to the Haskell type in
/// haskell-src/Concordium/Genesis/Data/Base.hs in concordium-base.
#[derive(Debug, Serialize)]
pub struct CoreGenesisParametersV0 {
    /// Nominal time of the genesis block.
    pub time:                    Timestamp,
    /// The duration of a slot.
    pub slot_duration:           SlotDuration,
    /// The epoch length in slots.
    pub epoch_length:            u64,
    /// The maximum energy per block.
    pub max_block_energy:        Energy,
    /// The finalization parameters.
    pub finalization_parameters: FinalizationParameters,
}

/// The core genesis parameters, the leadership election nonce and the chain
/// parameters (except the foundation account index).
///
/// Used to derive parsing for the genesis parameter section of the TOML config.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GenesisParametersConfigV1 {
    /// Leadership election nonce.
    pub leadership_election_nonce: LeadershipElectionNonce,
    #[serde(flatten)]
    pub core: CoreGenesisParametersConfigV1,
    pub chain: GenesisChainParametersV2,
}

/// The core genesis parameters. This corresponds to the Haskell type in
/// haskell-src/Concordium/Genesis/Data/BaseV1.hs in concordium-base.
#[derive(Debug, SerdeDeserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoreGenesisParametersConfigV1 {
    /// Nominal time of the genesis block.
    pub genesis_time:   Option<chrono::DateTime<chrono::Utc>>,
    /// Duration of an epoch.
    pub epoch_duration: SlotDuration,
}

impl TryFrom<CoreGenesisParametersConfigV1> for CoreGenesisParametersV1 {
    type Error = anyhow::Error;

    fn try_from(config: CoreGenesisParametersConfigV1) -> Result<Self, Self::Error> {
        let time = if let Some(date) = config.genesis_time {
            date.timestamp_millis()
        } else {
            chrono::Utc::now().timestamp_millis()
        };
        anyhow::ensure!(
            time >= 0,
            "Genesis time before unix epoch is not supported."
        );
        let genesis_time = Timestamp {
            millis: time as u64,
        };
        Ok(Self {
            genesis_time,
            epoch_duration: config.epoch_duration,
        })
    }
}

/// The core genesis parameters. This corresponds to the Haskell type in
/// haskell-src/Concordium/Genesis/Data/BaseV1.hs in concordium-base.
#[derive(Debug, Serialize, SerdeDeserialize)]
pub struct CoreGenesisParametersV1 {
    /// Nominal time of the genesis block.
    pub genesis_time:   Timestamp,
    /// Duration of an epoch.
    pub epoch_duration: SlotDuration,
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

/// The genesis state in chain parameters version 2. This corresponds to the
/// Haskell type `GenesisState` from haskell-src/Concordium/Genesis/Data/Base.hs
/// for those protocol versions having chain parameters version 2, currently
/// only P6.
#[derive(Debug)]
pub struct GenesisStateCPV2 {
    pub cryptographic_parameters:  GlobalContext<ArCurve>,
    pub identity_providers:        BTreeMap<IpIdentity, IpInfo<IpPairing>>,
    pub anonymity_revokers:        BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    pub update_keys:               UpdateKeysCollection<ChainParameterVersion2>,
    pub chain_parameters:          ChainParameters<ChainParameterVersion2>,
    pub leadership_election_nonce: LeadershipElectionNonce,
    pub accounts:                  Vec<GenesisAccountPublic>,
}

impl Serial for GenesisStateCPV2 {
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
        core:          CoreGenesisParametersV0,
        initial_state: GenesisStateCPV0,
    },
    P2 {
        core:          CoreGenesisParametersV0,
        initial_state: GenesisStateCPV0,
    },
    P3 {
        core:          CoreGenesisParametersV0,
        initial_state: GenesisStateCPV0,
    },
    P4 {
        core:          CoreGenesisParametersV0,
        initial_state: GenesisStateCPV1,
    },
    P5 {
        core:          CoreGenesisParametersV0,
        initial_state: GenesisStateCPV1,
    },
    P6 {
        core:          CoreGenesisParametersV1,
        initial_state: GenesisStateCPV2,
    },
}

impl GenesisData {
    pub fn hash(&self) -> BlockHash {
        let mut hasher = sha2::Sha256::new();
        Slot::from(0u64).serial(&mut hasher);
        match self {
            GenesisData::P1 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P1.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
            GenesisData::P2 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P2.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
            GenesisData::P3 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P3.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
            GenesisData::P4 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P4.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
            GenesisData::P5 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P5.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
            GenesisData::P6 {
                core,
                initial_state,
            } => {
                ProtocolVersion::P6.serial(&mut hasher);
                // tag of initial genesis
                0u8.serial(&mut hasher);
                core.serial(&mut hasher);
                initial_state.serial(&mut hasher);
            }
        }
        let bytes: [u8; 32] = hasher.finalize().into();
        bytes.into()
    }
}

pub fn make_genesis_data_cpv0(
    pv: ProtocolVersion,
    core: CoreGenesisParametersV0,
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
        ProtocolVersion::P5 => None,
        ProtocolVersion::P6 => None,
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
            GenesisData::P5 {
                core,
                initial_state,
            } => {
                7u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
            GenesisData::P6 {
                core,
                initial_state,
            } => {
                8u8.serial(out);
                // tag of initial genesis
                0u8.serial(out);
                core.serial(out);
                initial_state.serial(out)
            }
        }
    }
}

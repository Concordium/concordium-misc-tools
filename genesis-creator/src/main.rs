/// A command line tool for generating genesis files.
///
/// The tool has two modes: `generate` that can generate a new genesis, and
/// `assemble` that can produce a genesis from existing files (for example to
/// regenereate the Mainnet `genesis.dat`).
///
/// In both modes the tool takes a TOML configuration file that specify the
/// genesis. For details, see the README.
use anyhow::{anyhow, bail, ensure, Context};
use clap::Parser;
use concordium_rust_sdk::{
    common::{
        self as crypto_common,
        derive::{Serial, Serialize},
        types::{Amount, CredentialIndex, KeyIndex, KeyPair, Timestamp},
        Buffer, Deserial, Get, ParseResult, ReadBytesExt, SerdeDeserialize, SerdeSerialize, Serial,
        Versioned, VERSION_0,
    },
    id,
    id::{
        account_holder::compute_sharing_data,
        constants::{ArCurve, IpPairing},
        curve_arithmetic::{Curve, Value},
        types::{
            account_address_from_registration_id, mk_dummy_description, AccCredentialInfo,
            AccountAddress, AccountCredentialWithoutProofs, AccountKeys, ArData, ArIdentity,
            ArInfo, ChainArData, CredentialData, CredentialDeploymentCommitments,
            CredentialDeploymentValues, CredentialHolderInfo, GlobalContext, IpData, IpIdentity,
            IpInfo, Policy, PublicCredentialData, SignatureThreshold, YearMonth,
        },
    },
    types::{
        hashes::LeadershipElectionNonce, AccessStructure, AccountIndex, AccountThreshold,
        AuthorizationsV0, AuthorizationsV1, BakerAggregationVerifyKey, BakerCredentials,
        BakerElectionVerifyKey, BakerId, BakerKeyPairs, BakerSignatureVerifyKey, BlockHeight,
        ChainParameterVersion0, ChainParameterVersion1, ChainParameters, ChainParametersV0,
        ChainParametersV1, CooldownParameters, ElectionDifficulty, Energy, Epoch, ExchangeRate,
        HigherLevelAccessStructure, PoolParameters, ProtocolVersion, RewardParameters,
        SlotDuration, TimeParameters, UpdateKeyPair, UpdateKeysCollection,
        UpdateKeysCollectionSkeleton, UpdateKeysIndex, UpdateKeysThreshold, UpdatePublicKey,
    },
};
use gcd::Gcd;
use rayon::prelude::*;
use serde::de::{self, DeserializeOwned};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Struct for specifying the cryptographic parameters. Either a path to a file
/// with existing gryptographic parameters, or the genesis string from which the
/// cryptographic parameters should be generated.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum CryptoParamsConfig {
    #[serde(rename_all = "camelCase")]
    Existing {
        source: PathBuf,
    },
    #[serde(rename_all = "camelCase")]
    Generate {
        genesis_string: String,
    },
}

/// Struct for specifying one or more genesis anonymity revokers. Either a path
/// to a file with an existing anonymity revoker, or an id for which an
/// anonymity revoker should be generated freshly. If the `repeat` is `Some(n)`,
/// it specifies that `n` anonymity revokers should be generated freshly,
/// starting from the given id.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum AnonymityRevokerConfig {
    #[serde(rename_all = "camelCase")]
    Existing {
        source: PathBuf,
    },
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
enum IdentityProviderConfig {
    #[serde(rename_all = "camelCase")]
    Existing {
        source: PathBuf,
    },
    #[serde(rename_all = "camelCase")]
    Fresh {
        id:     id::types::IpIdentity,
        repeat: Option<u32>,
    },
}

/// Private genesis account data. When generating fresh accounts, these are
/// output as JSON files. When using existing accounts, these are instrad input
/// as JSON files. The format is the same as what is expected when importing
/// genesis accounts with concordium-client.
#[derive(SerdeDeserialize, SerdeSerialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenesisAccount {
    account_keys:          AccountKeys,
    aci:                   AccCredentialInfo<id::constants::ArCurve>,
    address:               AccountAddress,
    credentials: Versioned<
        BTreeMap<
            CredentialIndex,
            AccountCredentialWithoutProofs<id::constants::ArCurve, id::constants::AttributeKind>,
        >,
    >,
    encryption_public_key: id::elgamal::PublicKey<id::constants::ArCurve>,
    encryption_secret_key: id::elgamal::SecretKey<id::constants::ArCurve>,
}

/// Struct corresponding to the Haskell type `GenesisBaker` in
/// haskell-src/Concordium/Genesis/Account.hs in concordium-base.
#[derive(Serialize, SerdeSerialize, SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenesisBakerPublic {
    stake:                  Amount,
    restake_earnings:       bool,
    baker_id:               BakerId,
    election_verify_key:    BakerElectionVerifyKey,
    signature_verify_key:   BakerSignatureVerifyKey,
    aggregation_verify_key: BakerAggregationVerifyKey,
}

type GenesisCredentials = BTreeMap<
    CredentialIndex,
    AccountCredentialWithoutProofs<id::constants::ArCurve, id::constants::AttributeKind>,
>;

/// Struct corresponding to the Haskell type `GenesisAccount` in
/// haskell-src/Concordium/Genesis/Account.hs in concordium-base.
#[derive(Serialize, SerdeSerialize, SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenesisAccountPublic {
    address:           AccountAddress,
    account_threshold: AccountThreshold,
    #[map_size_length = 8]
    #[serde(deserialize_with = "deserialize_versioned_public_account")]
    credentials:       GenesisCredentials,
    balance:           Amount,
    baker:             Option<GenesisBakerPublic>,
}

fn deserialize_versioned_public_account<'de, D: de::Deserializer<'de>>(
    des: D,
) -> Result<GenesisCredentials, D::Error> {
    let versioned: Versioned<GenesisCredentials> =
        Versioned::<GenesisCredentials>::deserialize(des)?;
    Ok(versioned.value)
}

/// Struct for specifying one or more genesis accounts. Either a path to a file
/// with an existing account is given, or a fresh account should be generated
/// freshly.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum AccountConfig {
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
struct Level2UpdateConfig {
    authorized_keys: Vec<UpdateKeysIndex>,
    threshold:       UpdateKeysThreshold,
}

impl Level2UpdateConfig {
    pub fn access_structure(self, ctx: &Vec<UpdatePublicKey>) -> anyhow::Result<AccessStructure> {
        let num_given_keys = self.authorized_keys.len();
        let authorized_keys: BTreeSet<_> = self.authorized_keys.into_iter().collect();
        ensure!(authorized_keys.len() == num_given_keys, "Duplicate key index provided.");
        for key_idx in authorized_keys.iter() {
            ensure!(
                usize::from(u16::from(*key_idx)) < ctx.len(),
                "Key index {} does not specify a known update key.",
                key_idx
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
struct Level2KeysConfig {
    keys: Vec<HigherLevelKey>,
    emergency: Level2UpdateConfig,
    protocol: Level2UpdateConfig,
    election_difficulty: Level2UpdateConfig,
    euro_per_energy: Level2UpdateConfig,
    #[serde(rename = "microCCDPerEuro")]
    micro_ccd_per_euro: Level2UpdateConfig,
    foundation_account: Level2UpdateConfig,
    mint_distribution: Level2UpdateConfig,
    transaction_fee_distribution: Level2UpdateConfig,
    gas_rewards: Level2UpdateConfig,
    pool_parameters: Level2UpdateConfig,
    add_anonymity_revoker: Level2UpdateConfig,
    add_identity_provider: Level2UpdateConfig,
    // Optional because it is not needed in P1-P3,
    cooldown_parameters: Option<Level2UpdateConfig>,
    time_parameters: Option<Level2UpdateConfig>,
}

/// Struct holding the root or the level 1 keys, together with a threshold.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct HigherLevelKeysConfig {
    threshold: UpdateKeysThreshold,
    keys:      Vec<HigherLevelKey>,
}

/// Struct for specifying a key. Either a path to an existing key, or a `u32`
/// specifying how many keys should be generated freshly.
#[derive(SerdeDeserialize, Debug)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum HigherLevelKey {
    Existing {
        source: PathBuf,
    },
    Fresh {
        repeat: u32,
    },
}

/// Struct holding all the root, level 1 and level 2 keys.
#[derive(SerdeDeserialize, Debug)]
struct UpdateKeysConfig {
    root:   HigherLevelKeysConfig,
    level1: HigherLevelKeysConfig,
    level2: Level2KeysConfig,
}
/// Genesis chain parameters version 0. Contains all version 0 chain paramters
/// except for the foundation index.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenesisChainParametersV0 {
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
struct GenesisChainParametersV1 {
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
enum GenesisChainParameters {
    #[serde(rename = "v0")]
    V0(GenesisChainParametersV0),
    #[serde(rename = "v1")]
    V1(GenesisChainParametersV1),
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
        ensure!(numerator.gcd(denominator) == 1, "Numerator and denominator must be coprime.");
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
struct FinalizationParameters {
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

/// The core genesis parameters, the leadership election nonce and the chain
/// parameters (except the foundation index).
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct GenesisParameters {
    // Time at which the genesis will occur.
    genesis_time:              chrono::DateTime<chrono::Utc>,
    // Duration of a slot in milliseconds
    slot_duration:             SlotDuration,
    // Leadership election nonce.
    leadership_election_nonce: LeadershipElectionNonce,
    // Number of slots that go into an epoch.
    epoch_length:              u64,
    // Finalization parameters.
    finalization:              FinalizationParameters,
    // Max energy that is allowed for a block.
    max_block_energy:          Energy,
    chain:                     GenesisChainParameters,
}

impl GenesisParameters {
    pub fn to_core(&self) -> anyhow::Result<CoreGenesisParameters> {
        let time = self.genesis_time.timestamp_millis();
        ensure!(time >= 0, "Genesis time before unix epoch is not supported.");
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

/// For specifying where to ouput chain update keys, account keys, baker keys,
/// identity providers, anonymity revokers, cryptographic parameters and the
/// `genesis.dat` file. The `delete_existing` field specifies whether to delete
/// existing files before generation.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct OutputConfig {
    update_keys:              PathBuf,
    account_keys:             PathBuf,
    baker_keys:               PathBuf,
    identity_providers:       PathBuf,
    anonymity_revokers:       PathBuf,
    genesis:                  PathBuf,
    cryptographic_parameters: Option<PathBuf>,
    #[serde(default)]
    delete_existing:          bool,
}

/// Struct representing the configuration specified by the input TOML file.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Config {
    out: OutputConfig,
    protocol_version: ProtocolVersion,
    parameters: GenesisParameters,
    updates: UpdateKeysConfig,
    cryptographic_parameters: CryptoParamsConfig,
    anonymity_revokers: Vec<AnonymityRevokerConfig>,
    identity_providers: Vec<IdentityProviderConfig>,
    accounts: Vec<AccountConfig>,
}

/// Function for creating the cryptographic parameters (also called global
/// context). The arguments are
/// - global_out - if some, where to write the cryptographic parameters
/// - cfg - The configuration deciding whether to use existing cryptographic
///   parameters or
/// to generate the freshly.
///
/// The function returns `anyhow::Result`, which upon success will contain the
/// cryptographic parameters.
fn crypto_parameters(
    global_out: Option<PathBuf>,
    cfg: CryptoParamsConfig,
) -> anyhow::Result<GlobalContext<ArCurve>> {
    match cfg {
        CryptoParamsConfig::Existing {
            source,
        } => {
            let data = std::fs::read(&source).context(format!(
                "Could not read cryptographic parameters: {}",
                source.display()
            ))?;
            let data: Versioned<GlobalContext<ArCurve>> = serde_json::from_slice(&data)
                .context("Could not parse cryptographic parameters.")?;
            ensure!(
                data.version == 0.into(),
                "Incorrect version of cryptographic parameters. Expected 0, but got {}",
                data.version
            );
            Ok(data.value)
        }
        CryptoParamsConfig::Generate {
            genesis_string,
        } => {
            let ver_global: Versioned<GlobalContext<ArCurve>> = Versioned {
                version: VERSION_0,
                value:   GlobalContext::generate(genesis_string),
            };
            if let Some(out) = global_out {
                let mut path = out;
                path.push("cryptographic-parameters.json");
                std::fs::write(path, serde_json::to_string_pretty(&ver_global).unwrap())
                    .context("Unable to output account keys.")?;
            }
            Ok(ver_global.value)
        }
    }
}

/// Function for creating the genesis identity providers. The arguments are
/// - idp_out - where to write the identity providers
/// - cfgs - A vector of configurations, each deciding whether to use an
///   existing identity provider or
/// to generate one or more freshly.
///
/// For each generated anonymity revoker, the private identity provider data
/// will be written to a file. The function returns a in a `anyhow::Result`,
/// which upon success contains a `BTreeMap` with the public identity provider
/// infos.
fn identity_providers(
    idp_out: PathBuf,
    cfgs: Vec<IdentityProviderConfig>,
) -> anyhow::Result<BTreeMap<IpIdentity, IpInfo<IpPairing>>> {
    let mut csprng = rand::thread_rng();
    let mut out = BTreeMap::new();
    for cfg in cfgs {
        match cfg {
            IdentityProviderConfig::Existing {
                source,
            } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the identity provider file: {}",
                    source.display()
                ))?;
                let data: Versioned<IpInfo<IpPairing>> = serde_json::from_slice(&data)
                    .context("Could not parse the identity provider.")?;
                let ip_identity = data.value.ip_identity;
                if out.insert(data.value.ip_identity, data.value).is_some() {
                    bail!("Duplicate identity provider id {}", ip_identity);
                }
            }
            IdentityProviderConfig::Fresh {
                id,
                repeat,
            } => {
                let num = repeat.unwrap_or(1);
                for n in id.0..id.0 + num {
                    let ip_identity = IpIdentity::from(n);
                    ensure!(
                        !out.contains_key(&ip_identity),
                        "Duplicate identity provider {}",
                        ip_identity
                    );
                    let ip_description = mk_dummy_description(format!("Generated IP {}", n));
                    // using 30 as the key capacity. That is enough given the current list of
                    // attributes.
                    let ip_secret_key =
                        concordium_rust_sdk::id::ps_sig::SecretKey::<IpPairing>::generate(
                            30,
                            &mut csprng,
                        );
                    let ip_verify_key = (&ip_secret_key).into();
                    let ip_cdi_kp = ed25519_dalek::Keypair::generate(&mut csprng);
                    let ip_cdi_verify_key = ip_cdi_kp.public;
                    let ip_data = IpData {
                        public_ip_info: IpInfo {
                            ip_identity,
                            ip_description,
                            ip_verify_key,
                            ip_cdi_verify_key,
                        },
                        ip_secret_key,
                        ip_cdi_secret_key: ip_cdi_kp.secret,
                    };
                    {
                        let mut path = idp_out.clone();
                        path.push(format!("ip-data-{}.json", n));
                        std::fs::write(path, serde_json::to_string_pretty(&ip_data).unwrap())
                            .context("Unable to write the identity provider.")?;
                    }
                    out.insert(ip_identity, ip_data.public_ip_info);
                }
            }
        }
    }
    let ver_idps = Versioned {
        version: VERSION_0,
        value:   out,
    };
    {
        let mut path = idp_out;
        path.push("identity-providers.json");
        std::fs::write(path, serde_json::to_string_pretty(&ver_idps).unwrap())
            .context("Unable to write the identity providers.")?;
    }
    Ok(ver_idps.value)
}

/// Function for creating the genesis anonymity revokers. The arguments are
/// - ars_out - where to write the anonymity revokers
/// - cfgs - A vector of configurations, each deciding whether to use an
///   existing anonymity revoker or
/// to generate one or more freshly.
///
/// For each generated anonymity revoker, the private anonymity revoker data
/// will be written to a file. The function returns a in a `anyhow::Result`,
/// which upon success contains a `BTreeMap` with the public anonymity revoker
/// infos.
fn anonymity_revokers(
    ars_out: PathBuf,
    params: &GlobalContext<ArCurve>,
    cfgs: Vec<AnonymityRevokerConfig>,
) -> anyhow::Result<BTreeMap<ArIdentity, ArInfo<ArCurve>>> {
    let mut csprng = rand::thread_rng();
    let mut out = BTreeMap::new();
    for cfg in cfgs {
        match cfg {
            AnonymityRevokerConfig::Existing {
                source,
            } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the identity provider file: {}",
                    source.display()
                ))?;
                let data: Versioned<ArInfo<ArCurve>> = serde_json::from_slice(&data)
                    .context("Could not parse the anonymity revoker.")?;
                let ar_identity = data.value.ar_identity;
                if out.insert(ar_identity, data.value).is_some() {
                    bail!("Duplicate anonymity revoker id {}", ar_identity);
                }
            }
            AnonymityRevokerConfig::Fresh {
                id,
                repeat,
            } => {
                let num = repeat.unwrap_or(1);
                for n in u32::from(id)..u32::from(id) + num {
                    let ar_identity = ArIdentity::try_from(n).map_err(|_| {
                        anyhow::anyhow!("Invalid anonymity revoker ID would be generated.")
                    })?;
                    ensure!(
                        !out.contains_key(&ar_identity),
                        "Duplicate anonymity revoker {}",
                        ar_identity
                    );
                    let ar_description = mk_dummy_description(format!("Generated AR {}", n));
                    let ar_secret_key =
                        id::elgamal::SecretKey::generate(params.elgamal_generator(), &mut csprng);
                    let ar_data = ArData {
                        public_ar_info: ArInfo {
                            ar_identity,
                            ar_description,
                            ar_public_key: (&ar_secret_key).into(),
                        },
                        ar_secret_key,
                    };
                    {
                        let mut path = ars_out.clone();
                        path.push(format!("ar-data-{}.json", n));
                        std::fs::write(path, serde_json::to_string_pretty(&ar_data).unwrap())
                            .context("Unable to write the anonymity revoker.")?;
                    }
                    out.insert(ar_identity, ar_data.public_ar_info);
                }
            }
        }
    }
    let ver_ars = Versioned {
        version: VERSION_0,
        value:   out,
    };
    {
        let mut path = ars_out;
        path.push("anonymity-revokers.json");
        std::fs::write(path, serde_json::to_string_pretty(&ver_ars).unwrap())
            .context("Unable to write the anonymity revokers.")?;
    }
    Ok(ver_ars.value)
}

/// Function for creating a vector of root, level 1 or level 2 keys, where each
/// key is either generated freshly or read from a file. The arguments are
/// - ctx - description of the keys, e.g. "root", "level1" or "level2".
/// - root_out - the directory in which the keys whould be placed.
/// - csprng - a cryptographically secure random number generator.
/// - key_cfgs - A vector of confiurations, each deciding whether to generate or
///   to read from a file.
///
/// For each generated key, the private keypair will be written to a file.
/// The function returns a `anyhow::Result`, which upon success will contain a
/// vector with the public keys.
fn read_or_generate_update_keys<R: rand::Rng + rand::CryptoRng>(
    ctx: &str,
    root_out: &Path,
    csprng: &mut R,
    key_cfgs: &[HigherLevelKey],
) -> anyhow::Result<Vec<UpdatePublicKey>> {
    let mut out = Vec::new();
    for key_cfg in key_cfgs {
        match key_cfg {
            HigherLevelKey::Existing {
                source,
            } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the {} key: {}",
                    ctx,
                    source.display()
                ))?;
                let key: UpdatePublicKey = serde_json::from_slice(&data)
                    .context(format!("Could not parse the {} key.", ctx))?;
                out.push(key);
            }
            HigherLevelKey::Fresh {
                repeat,
            } => {
                for _ in 0..*repeat {
                    let new_key = UpdateKeyPair::generate(csprng);
                    let mut path = root_out.to_path_buf();
                    path.push(format!("{}-key-{}.json", ctx, out.len()));
                    std::fs::write(path, serde_json::to_string_pretty(&new_key).unwrap())
                        .context(format!("Unable to write {} key.", ctx))?;
                    out.push(new_key.public);
                }
            }
        }
    }
    Ok(out)
}

/// Function for creating a version 0 `UpdateKeysCollection` containing all
/// root, level 1 and level 2 keys. The arguments are
/// - updates_out - where to put all chain update keys
/// - update_cfg - the configuration specifying all keys and thresholds
///
/// The function returns a `anyhow::Result`, whic upon success contains the
/// version 0 `UpdateKeysCollection`. NB: To be used only in chain parameters
/// version 0.
fn updates_v0(
    updates_out: PathBuf,
    update_cfg: UpdateKeysConfig,
) -> anyhow::Result<UpdateKeysCollection<ChainParameterVersion0>> {
    let mut csprng = rand::thread_rng();
    let root_keys =
        read_or_generate_update_keys("root", &updates_out, &mut csprng, &update_cfg.root.keys)?;
    ensure!(
        usize::from(u16::from(update_cfg.root.threshold)) <= root_keys.len(),
        "The number of root keys ({}) is less than the threshold ({}).",
        root_keys.len(),
        update_cfg.root.threshold
    );

    let level1_keys =
        read_or_generate_update_keys("level1", &updates_out, &mut csprng, &update_cfg.level1.keys)?;
    ensure!(
        usize::from(u16::from(update_cfg.level1.threshold)) <= level1_keys.len(),
        "The number of level_1 keys ({}) is less than the threshold ({}).",
        level1_keys.len(),
        update_cfg.level1.threshold
    );

    let level2_keys =
        read_or_generate_update_keys("level2", &updates_out, &mut csprng, &update_cfg.level2.keys)?;
    ensure!(!level2_keys.is_empty(), "There must be at least one level 2 key.",);

    let level2 = update_cfg.level2;
    let emergency = level2.emergency.access_structure(&level2_keys)?;
    let protocol = level2.protocol.access_structure(&level2_keys)?;
    let election_difficulty = level2.election_difficulty.access_structure(&level2_keys)?;
    let euro_per_energy = level2.euro_per_energy.access_structure(&level2_keys)?;
    let micro_gtu_per_euro = level2.micro_ccd_per_euro.access_structure(&level2_keys)?;
    let foundation_account = level2.foundation_account.access_structure(&level2_keys)?;
    let mint_distribution = level2.mint_distribution.access_structure(&level2_keys)?;
    let transaction_fee_distribution =
        level2.transaction_fee_distribution.access_structure(&level2_keys)?;
    let param_gas_rewards = level2.gas_rewards.access_structure(&level2_keys)?;
    let pool_parameters = level2.pool_parameters.access_structure(&level2_keys)?;
    let add_anonymity_revoker = level2.add_anonymity_revoker.access_structure(&level2_keys)?;
    let add_identity_provider = level2.add_identity_provider.access_structure(&level2_keys)?;

    let level_2_keys = AuthorizationsV0 {
        keys: level2_keys,
        emergency,
        protocol,
        election_difficulty,
        euro_per_energy,
        micro_gtu_per_euro,
        foundation_account,
        mint_distribution,
        transaction_fee_distribution,
        param_gas_rewards,
        pool_parameters,
        add_anonymity_revoker,
        add_identity_provider,
    };

    let uks = UpdateKeysCollectionSkeleton {
        root_keys: HigherLevelAccessStructure {
            keys:      root_keys,
            threshold: update_cfg.root.threshold,
            _phantom:  Default::default(),
        },
        level_1_keys: HigherLevelAccessStructure {
            keys:      level1_keys,
            threshold: update_cfg.level1.threshold,
            _phantom:  Default::default(),
        },
        level_2_keys,
    };

    {
        let mut path = updates_out.to_path_buf();
        path.push("governance-keys.json");
        std::fs::write(path, serde_json::to_string_pretty(&uks).unwrap())
            .context("Unable to write authorizations.")?;
    }

    Ok(uks)
}

/// Function for creating a version 1 `UpdateKeysCollection` containing all
/// root, level 1 and level 2 keys. The arguments are
/// - updates_out - where to put all chain update keys
/// - update_cfg - the configuration specifying all keys and thresholds
///
/// The function returns a `anyhow::Result`, whic upon success contains the
/// version 1 `UpdateKeysCollection`. NB: To be used only in chain parameters
/// version 1.
fn updates_v1(
    updates_out: PathBuf,
    update_cfg: UpdateKeysConfig,
) -> anyhow::Result<UpdateKeysCollection<ChainParameterVersion1>> {
    let mut csprng = rand::thread_rng();
    let root_keys =
        read_or_generate_update_keys("root", &updates_out, &mut csprng, &update_cfg.root.keys)?;
    ensure!(
        usize::from(u16::from(update_cfg.root.threshold)) <= root_keys.len(),
        "The number of root keys ({}) is less than the threshold ({}).",
        root_keys.len(),
        update_cfg.root.threshold
    );

    let level1_keys =
        read_or_generate_update_keys("level1", &updates_out, &mut csprng, &update_cfg.level1.keys)?;
    ensure!(
        usize::from(u16::from(update_cfg.level1.threshold)) <= level1_keys.len(),
        "The number of level_1 keys ({}) is less than the threshold ({}).",
        level1_keys.len(),
        update_cfg.level1.threshold
    );

    let level2_keys =
        read_or_generate_update_keys("level2", &updates_out, &mut csprng, &update_cfg.level2.keys)?;
    ensure!(!level2_keys.is_empty(), "There must be at least one level 2 key.",);

    let level2 = update_cfg.level2;
    let emergency = level2.emergency.access_structure(&level2_keys)?;
    let protocol = level2.protocol.access_structure(&level2_keys)?;
    let election_difficulty = level2.election_difficulty.access_structure(&level2_keys)?;
    let euro_per_energy = level2.euro_per_energy.access_structure(&level2_keys)?;
    let micro_gtu_per_euro = level2.micro_ccd_per_euro.access_structure(&level2_keys)?;
    let foundation_account = level2.foundation_account.access_structure(&level2_keys)?;
    let mint_distribution = level2.mint_distribution.access_structure(&level2_keys)?;
    let transaction_fee_distribution =
        level2.transaction_fee_distribution.access_structure(&level2_keys)?;
    let param_gas_rewards = level2.gas_rewards.access_structure(&level2_keys)?;
    let pool_parameters = level2.pool_parameters.access_structure(&level2_keys)?;
    let add_anonymity_revoker = level2.add_anonymity_revoker.access_structure(&level2_keys)?;
    let add_identity_provider = level2.add_identity_provider.access_structure(&level2_keys)?;
    let cooldown_parameters = level2
        .cooldown_parameters
        .ok_or_else(|| anyhow!("Cooldown parameters missing"))?
        .access_structure(&level2_keys)?;
    let time_parameters = level2
        .time_parameters
        .ok_or_else(|| anyhow!("Time parameters missing"))?
        .access_structure(&level2_keys)?;

    let v0 = AuthorizationsV0 {
        keys: level2_keys,
        emergency,
        protocol,
        election_difficulty,
        euro_per_energy,
        micro_gtu_per_euro,
        foundation_account,
        mint_distribution,
        transaction_fee_distribution,
        param_gas_rewards,
        pool_parameters,
        add_anonymity_revoker,
        add_identity_provider,
    };
    let level_2_keys = AuthorizationsV1 {
        v0,
        cooldown_parameters,
        time_parameters,
    };

    let uks = UpdateKeysCollectionSkeleton {
        root_keys: HigherLevelAccessStructure {
            keys:      root_keys,
            threshold: update_cfg.root.threshold,
            _phantom:  Default::default(),
        },
        level_1_keys: HigherLevelAccessStructure {
            keys:      level1_keys,
            threshold: update_cfg.level1.threshold,
            _phantom:  Default::default(),
        },
        level_2_keys,
    };

    {
        let mut path = updates_out.to_path_buf();
        path.push("governance-keys.json");
        std::fs::write(path, serde_json::to_string_pretty(&uks).unwrap())
            .context("Unable to write authorizations.")?;
    }

    Ok(uks)
}

/// Function for creating a vector of genesis accounts, where each key is either
/// generated freshly or read from a file. The arguments are
/// - baker_keys_out - where to put baker keys for those accounts that are
///   bakers.
/// - account_keys_out - where to put the account keys.
/// - params - the cryptographic parameters.
/// - ars - the anonymity revokers
/// - cfgs - A vector of confiurations, each deciding whether to generate or to
///   read from a file.
///
/// For each generated account, the private account information will be written
/// to a file. The function returns a `anyhow::Result`, which upon success will
/// contain the foundation account index together with a vector with the public
/// account parts.
fn accounts(
    baker_keys_out: PathBuf,
    account_keys_out: PathBuf,
    params: &GlobalContext<ArCurve>,
    ars: &BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    cfgs: Vec<AccountConfig>,
) -> anyhow::Result<(AccountIndex, Vec<GenesisAccountPublic>)> {
    let mut foundation_index = None;
    let mut idx: u64 = 0;

    let mut gas = Vec::new();

    let mut csprng = rand::thread_rng();

    for cfg in cfgs {
        match cfg {
            AccountConfig::Existing {
                source,
                foundation,
                balance,
                stake,
                restake_earnings,
                baker_keys,
            } => {
                if foundation_index.is_some() && foundation {
                    bail!(
                        "There are two accounts marked as foundation accounts. That will not work."
                    );
                }
                if foundation {
                    foundation_index = Some(AccountIndex::from(idx));
                }
                let ga: GenesisAccount = serde_json::from_slice(
                    &std::fs::read(source).context("Could not read existing account file.")?,
                )
                .context("Could not parse existing account file.")?;

                let baker = if let Some(stake) = stake {
                    let baker_id = BakerId::from(AccountIndex::from(idx));
                    let creds = {
                        if let Some(baker_keys) = baker_keys {
                            let creds: BakerCredentials = serde_json::from_slice(
                                &std::fs::read(baker_keys)
                                    .context("Could not read existing baker credentials file.")?,
                            )
                            .context("Could not parse existing baker credentials file.")?;
                            ensure!(
                                creds.baker_id == baker_id,
                                "Baker credential does not match the assign account index."
                            );
                            creds
                        } else {
                            let creds = BakerCredentials::new(
                                baker_id,
                                BakerKeyPairs::generate(&mut csprng),
                            );
                            let mut path = baker_keys_out.clone();
                            path.push(format!("baker-{}-credentials.json", idx));
                            std::fs::write(path, serde_json::to_string_pretty(&creds).unwrap())
                                .context("Unable to output baker keys.")?;
                            creds
                        }
                    };
                    let gb = GenesisBakerPublic {
                        aggregation_verify_key: creds.keys.aggregation_verify,
                        election_verify_key: creds.keys.election_verify,
                        signature_verify_key: creds.keys.signature_verify,
                        baker_id,
                        stake,
                        restake_earnings,
                    };
                    Some(gb)
                } else {
                    None
                };

                let ga_public = GenesisAccountPublic {
                    address: ga.address,
                    account_threshold: ga.account_keys.threshold.0.try_into()?,
                    credentials: ga.credentials.value,
                    balance,
                    baker,
                };
                gas.push(ga_public);
                idx += 1;
            }
            AccountConfig::Fresh {
                repeat,
                stake,
                restake_earnings,
                balance,
                template,
                num_keys,
                threshold,
                identity_provider,
                foundation,
            } => {
                if foundation_index.is_some() && foundation {
                    bail!(
                        "There are two accounts marked as foundation accounts. That will not work."
                    );
                }
                if foundation {
                    foundation_index = Some(AccountIndex::from(idx));
                }

                let num = repeat.unwrap_or(1);
                ensure!(num > 0, "repeat cannot be 0");

                let num_keys = num_keys.unwrap_or(1);
                let threshold = threshold.unwrap_or(SignatureThreshold(1));

                ensure!(
                    num_keys >= u8::from(threshold),
                    "Signature threshold must be at most the number of keys."
                );

                let mut gas_public = (idx..idx + u64::from(num))
                    .into_par_iter()
                    .map(|n| {
                        let mut csprng = rand::thread_rng();
                        let prf_key = concordium_rust_sdk::id::dodis_yampolskiy_prf::SecretKey::<
                            ArCurve,
                        >::generate_non_zero(&mut csprng);
                        let prf_exponent = prf_key.prf_exponent(0)?;
                        let cred_id: ArCurve = prf_key.prf(params.elgamal_generator(), 0)?;

                        let created_at = YearMonth::now();
                        let valid_to =
                            YearMonth::new(created_at.year + 5, created_at.month).unwrap();

                        let id_cred_sec = Value::<ArCurve>::generate_non_zero(&mut csprng);

                        let ar_threshold = std::cmp::max(1, u8::try_from(ars.len() - 1)?);

                        let sharing_data = compute_sharing_data(
                            &id_cred_sec,
                            ars,
                            ar_threshold.try_into().unwrap(),
                            &params.on_chain_commitment_key,
                        );

                        let ar_data = sharing_data
                            .0
                            .into_iter()
                            .map(|sad| {
                                (sad.ar.ar_identity, ChainArData {
                                    enc_id_cred_pub_share: sad.encrypted_share,
                                })
                            })
                            .collect();

                        let (account_keys, cred_key_info) = {
                            let mut cred_keys = BTreeMap::new();
                            for i in 0..num_keys {
                                cred_keys.insert(KeyIndex(i), KeyPair::generate(&mut csprng));
                            }
                            let cred_data = CredentialData {
                                keys: cred_keys,
                                threshold,
                            };
                            let cred_key_info = cred_data.get_cred_key_info();
                            (AccountKeys::from(cred_data), cred_key_info)
                        };

                        let acc_cred = AccountCredentialWithoutProofs::Normal {
                            cdv:         CredentialDeploymentValues {
                                cred_key_info,
                                cred_id,
                                ip_identity: identity_provider,
                                threshold: 1u8.try_into().unwrap(),
                                ar_data,
                                policy: Policy {
                                    valid_to,
                                    created_at,
                                    policy_vec: BTreeMap::new(),
                                    _phantom: Default::default(),
                                },
                            },
                            commitments: CredentialDeploymentCommitments {
                                cmm_prf: params
                                    .on_chain_commitment_key
                                    .commit(&prf_key, &mut csprng)
                                    .0,
                                cmm_cred_counter: params
                                    .on_chain_commitment_key
                                    .commit(
                                        &Value::<ArCurve>::new(ArCurve::scalar_from_u64(0)),
                                        &mut csprng,
                                    )
                                    .0,
                                cmm_max_accounts: params
                                    .on_chain_commitment_key
                                    .commit(
                                        &Value::<ArCurve>::new(ArCurve::scalar_from_u64(1)),
                                        &mut csprng,
                                    )
                                    .0,
                                cmm_attributes: BTreeMap::new(),
                                cmm_id_cred_sec_sharing_coeff: sharing_data.1,
                            },
                        };
                        let encryption_secret_key = concordium_rust_sdk::id::elgamal::SecretKey {
                            generator: *params.elgamal_generator(),
                            scalar:    prf_exponent,
                        };

                        let aci = AccCredentialInfo {
                            cred_holder_info: CredentialHolderInfo {
                                id_cred: id_cred_sec.into(),
                            },
                            prf_key,
                        };

                        let ga = GenesisAccount {
                            account_keys,
                            aci,
                            address: account_address_from_registration_id(&cred_id),
                            credentials: Versioned::new(
                                VERSION_0,
                                [(
                                    CredentialIndex {
                                        index: 0,
                                    },
                                    acc_cred,
                                )]
                                .into_iter()
                                .collect(),
                            ),
                            encryption_public_key: (&encryption_secret_key).into(),
                            encryption_secret_key,
                        };

                        {
                            let mut path = account_keys_out.clone();
                            path.push(format!("{}-{}.json", template, n));
                            std::fs::write(path, serde_json::to_string_pretty(&ga).unwrap())
                                .context("Unable to output account keys.")?;
                        }

                        let baker = if let Some(stake) = stake {
                            ensure!(
                                stake <= balance,
                                "Initial stake must not be above the initial balance."
                            );
                            let keys = BakerKeyPairs::generate(&mut csprng);
                            let baker_id = BakerId::from(AccountIndex::from(n));
                            let creds = BakerCredentials::new(baker_id, keys);

                            let mut path = baker_keys_out.clone();
                            path.push(format!("baker-{}-credentials.json", n));
                            std::fs::write(path, serde_json::to_string_pretty(&creds).unwrap())
                                .context("Unable to output baker keys.")?;

                            let gb = GenesisBakerPublic {
                                aggregation_verify_key: creds.keys.aggregation_verify,
                                election_verify_key: creds.keys.election_verify,
                                signature_verify_key: creds.keys.signature_verify,
                                baker_id,
                                stake,
                                restake_earnings,
                            };
                            Some(gb)
                        } else {
                            None
                        };

                        Ok(GenesisAccountPublic {
                            address: account_address_from_registration_id(&cred_id),
                            account_threshold: 1.try_into().unwrap(),
                            credentials: ga.credentials.value,
                            balance,
                            baker,
                        })
                    })
                    .collect::<anyhow::Result<_>>()?;
                gas.append(&mut gas_public);
                idx += u64::from(num);
            }
        }
    }
    {
        let mut path = account_keys_out;
        path.push("accounts.json");
        std::fs::write(path, serde_json::to_string_pretty(&gas).unwrap())
            .context("Unable to output accounts.")?;
    }
    if let Some(foundation_index) = foundation_index {
        Ok((foundation_index, gas))
    } else {
        bail!("Exactly one account must be designated as a foundation account.")
    }
}

/// The core genesis parameters. This corresponds to the Haskell type in
/// haskell-src/Concordium/Genesis/Data/Base.hs in concordium-base.
#[derive(Debug, Serialize)]
struct CoreGenesisParameters {
    time:                    Timestamp,
    slot_duration:           SlotDuration,
    epoch_length:            u64,
    max_block_energy:        Energy,
    finalization_parameters: FinalizationParameters,
}

/// The genesis state in chain parameters version 0. This corresponds to the
/// Haskell type `GenesisState` from haskell-src/Concordium/Genesis/Data/Base.hs
/// for those protocol versions having chain parameters version 0.
#[derive(Debug)]
struct GenesisStateCPV0 {
    cryptographic_parameters:  GlobalContext<ArCurve>,
    identity_providers:        BTreeMap<IpIdentity, IpInfo<IpPairing>>,
    anonymity_revokers:        BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    update_keys:               UpdateKeysCollection<ChainParameterVersion0>,
    chain_parameters:          ChainParameters<ChainParameterVersion0>,
    leadership_election_nonce: LeadershipElectionNonce,
    accounts:                  Vec<GenesisAccountPublic>,
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
struct GenesisStateCPV1 {
    cryptographic_parameters:  GlobalContext<ArCurve>,
    identity_providers:        BTreeMap<IpIdentity, IpInfo<IpPairing>>,
    anonymity_revokers:        BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    update_keys:               UpdateKeysCollection<ChainParameterVersion1>,
    chain_parameters:          ChainParameters<ChainParameterVersion1>,
    leadership_election_nonce: LeadershipElectionNonce,
    accounts:                  Vec<GenesisAccountPublic>,
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
enum GenesisData {
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

fn make_genesis_data_cpv0(
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

/// Check whether the directory exists, and either fail or delete it depending
/// on the value of the `delete_existing` flag.
fn check_and_create_dir(delete_existing: bool, path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        if delete_existing {
            std::fs::remove_dir_all(path).context("Failed to remove the existing directory.")?;
        } else {
            bail!("Supplied output path {} already exists.", path.display());
        }
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// Configuration struct for specifying protocol version, the genesis
/// parameters, the foundation account and where to find genesis accounts,
/// anonymity revokers, identity providers, cryptographic parameters and where
/// to output the genesis data file.
#[derive(SerdeDeserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AssembleGenesisConfig {
    protocol_version:   ProtocolVersion,
    parameters:         GenesisParameters,
    foundation_account: AccountAddress,
    /// A file with a list of accounts that should be assembled into
    /// genesis.
    accounts:           PathBuf,
    /// A file with a list of anonymity revokers that should be assembled
    /// into genesis.
    ars:                PathBuf,
    /// A file with a list of identity providers that should be assembled
    /// into genesis.
    idps:               PathBuf,
    /// A file pointing to the cryptographic parameters to be put into
    /// genesis.
    global:             PathBuf,
    /// A file pointing to the governance (root, level 1, and level 2) keys.
    governance_keys:    PathBuf,
    /// Location where to output the genesis block.
    genesis_out:        PathBuf,
}

/// For defining the two modes, generate and assemble.
#[derive(clap::Subcommand, Debug)]
#[clap(author, version, about)]
enum GenesisCreatorCommand {
    Generate {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: PathBuf,
    },
    Assemble {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config: PathBuf,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct GenesisCreator {
    #[clap(subcommand)]
    action: GenesisCreatorCommand,
}

fn read_json<S: DeserializeOwned>(path: &Path) -> anyhow::Result<S> {
    let data_value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(path).context("Could not read existing account file.")?,
    )?;
    let data: S = serde_json::from_value(data_value)?;
    Ok(data)
}

/// Interpret the second path as relative to the first. If the second path is
/// absolute then return it.
fn make_relative(f1: &Path, f2: &Path) -> anyhow::Result<PathBuf> {
    let mut root = f1.parent().context("Origin is not a file.")?.to_path_buf();
    root.push(f2);
    Ok(root)
}

/// Function for assembling the genesis data file given a path to TOML file that
/// can be parsed as a `AssembleGenesisConfig`. Upon success it writes the
/// genesis data to the a file and returns `Ok(())`.
fn handle_assemble(config_path: &Path) -> anyhow::Result<()> {
    let config_source =
        std::fs::read(config_path).context("Unable to read the configuration file.")?;
    let config: AssembleGenesisConfig =
        toml::from_slice(&config_source).context("Unable to parse the configuration file.")?;
    // TODO: Make paths relative to the config file.
    let accounts: Vec<GenesisAccountPublic> =
        read_json(&make_relative(config_path, &config.accounts)?)?;
    let global = read_json::<Versioned<_>>(&make_relative(config_path, &config.global)?)?;
    let idps = read_json::<Versioned<_>>(&make_relative(config_path, &config.idps)?)?;
    let ars = read_json::<Versioned<_>>(&make_relative(config_path, &config.ars)?)?;

    let idx = accounts
        .iter()
        .zip(0u64..)
        .find_map(|(acc, i)| {
            if acc.address == config.foundation_account {
                Some(i)
            } else {
                None
            }
        })
        .context("Cannot find foundation account.")?;

    let core = config.parameters.to_core()?;
    match config.parameters.chain {
        GenesisChainParameters::V0(params) => {
            let update_keys = read_json(&make_relative(config_path, &config.governance_keys)?)?;
            let initial_state = GenesisStateCPV0 {
                cryptographic_parameters: global.value,
                identity_providers: idps.value,
                anonymity_revokers: ars.value,
                update_keys,
                chain_parameters: params.chain_parameters(AccountIndex::from(idx)),
                leadership_election_nonce: config.parameters.leadership_election_nonce,
                accounts,
            };
            let genesis = make_genesis_data_cpv0(config.protocol_version, core, initial_state)
                .context("P4 does not have CPV0")?;
            {
                let mut out = Vec::new();
                genesis.serial(&mut out);
                std::fs::write(make_relative(config_path, &config.genesis_out)?, out)
                    .context("Unable to write genesis.")?;
            }
        }
        GenesisChainParameters::V1(params) => {
            let update_keys = read_json(&make_relative(config_path, &config.governance_keys)?)?;
            let initial_state = GenesisStateCPV1 {
                cryptographic_parameters: global.value,
                identity_providers: idps.value,
                anonymity_revokers: ars.value,
                update_keys,
                chain_parameters: params.chain_parameters(AccountIndex::from(idx)),
                leadership_election_nonce: config.parameters.leadership_election_nonce,
                accounts,
            };
            let genesis = GenesisData::P4 {
                core,
                initial_state,
            };
            {
                let mut out = Vec::new();
                genesis.serial(&mut out);
                std::fs::write(make_relative(config_path, &config.genesis_out)?, out)
                    .context("Unable to write genesis.")?;
            }
        }
    }
    Ok(())
}

/// Function for generating a new genesis data file given a path to TOML file
/// that can be parsed as a `Config`. Upon success it writes the genesis data to
/// the a file and writes all relevant private data to their desired locations
/// and returns `Ok(())`.
fn handle_generate(config_path: &Path) -> anyhow::Result<()> {
    let config_source = std::fs::read(config_path).context(
        "Unable to read the configuration
    file.",
    )?;
    let config: Config = toml::from_slice(&config_source).context(
        "Unable to parse the
    configuration file.",
    )?;
    println!("{:#?}", config);

    check_and_create_dir(config.out.delete_existing, &config.out.account_keys)?;
    check_and_create_dir(config.out.delete_existing, &config.out.update_keys)?;
    check_and_create_dir(config.out.delete_existing, &config.out.identity_providers)?;
    check_and_create_dir(config.out.delete_existing, &config.out.anonymity_revokers)?;
    check_and_create_dir(config.out.delete_existing, &config.out.baker_keys)?;
    if let Some(global) = &config.out.cryptographic_parameters {
        check_and_create_dir(config.out.delete_existing, global)?;
    }

    let core = config.parameters.to_core()?;
    let cryptographic_parameters =
        crypto_parameters(config.out.cryptographic_parameters, config.cryptographic_parameters)?;
    let identity_providers =
        identity_providers(config.out.identity_providers, config.identity_providers)?;
    let anonymity_revokers = anonymity_revokers(
        config.out.anonymity_revokers,
        &cryptographic_parameters,
        config.anonymity_revokers,
    )?;
    let (foundation_idx, accounts) = accounts(
        config.out.baker_keys,
        config.out.account_keys,
        &cryptographic_parameters,
        &anonymity_revokers,
        config.accounts,
    )?;
    let genesis = match config.protocol_version {
        ProtocolVersion::P1 | ProtocolVersion::P2 | ProtocolVersion::P3 => {
            let params = match config.parameters.chain {
                GenesisChainParameters::V0(params) => params,
                GenesisChainParameters::V1(_) => {
                    bail!(format!(
                        "Protocol version {} supports only chain parameters
    version 0.",
                        config.protocol_version
                    ))
                }
            };
            let update_keys = updates_v0(config.out.update_keys, config.updates)?;
            let initial_state = GenesisStateCPV0 {
                cryptographic_parameters,
                identity_providers,
                anonymity_revokers,
                update_keys,
                chain_parameters: params.chain_parameters(foundation_idx),
                leadership_election_nonce: config.parameters.leadership_election_nonce,
                accounts,
            };
            make_genesis_data_cpv0(config.protocol_version, core, initial_state)
                .context("Chain parameters version 0 should not be used in P4")?
            // Should go well since we know we are not in P4.
        }
        ProtocolVersion::P4 => {
            let core = config.parameters.to_core()?;
            let params = match config.parameters.chain {
                GenesisChainParameters::V1(params) => params,
                GenesisChainParameters::V0(_) => {
                    bail!(format!("Protocol version P4 supports only chain parameters version 1."))
                }
            };
            let update_keys = updates_v1(config.out.update_keys, config.updates)?;
            let initial_state = GenesisStateCPV1 {
                cryptographic_parameters,
                identity_providers,
                anonymity_revokers,
                update_keys,
                chain_parameters: params.chain_parameters(foundation_idx),
                leadership_election_nonce: config.parameters.leadership_election_nonce,
                accounts,
            };
            GenesisData::P4 {
                core,
                initial_state,
            }
        }
    };
    {
        let mut out = Vec::new();
        genesis.serial(&mut out);
        std::fs::write(config.out.genesis, out).context(
            "Unable to write
genesis.",
        )?;
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    let args = GenesisCreator::parse();

    println!("{:#?}", args);

    match &args.action {
        GenesisCreatorCommand::Generate {
            config,
        } => handle_generate(config),
        GenesisCreatorCommand::Assemble {
            config,
        } => handle_assemble(config),
    }
}

// TODO: Deny unused fields.
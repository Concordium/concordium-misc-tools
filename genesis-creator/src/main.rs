/// A command line tool for generating genesis files.
///
/// The tool has two modes: `generate` that can generate a new genesis,
/// potentially reusing some files/keys from the previously generated genesis,
/// and `assemble` that can produce a genesis from existing files (for example
/// to regenereate the Mainnet `genesis.dat`).
///
/// In both modes the tool takes a TOML configuration file that specifies the
/// genesis. For details, see the README.
use anyhow::{anyhow, bail, ensure, Context};
use clap::Parser;
use concordium_rust_sdk::{
    common::{
        types::{CredentialIndex, KeyIndex, KeyPair},
        Serial, Versioned, VERSION_0,
    },
    id,
    id::{
        account_holder::compute_sharing_data,
        constants::{ArCurve, IpPairing},
        curve_arithmetic::{Curve, Value},
        types::{
            account_address_from_registration_id, mk_dummy_description, AccCredentialInfo,
            AccountCredentialWithoutProofs, AccountKeys, ArData, ArIdentity, ArInfo, ChainArData,
            CredentialData, CredentialDeploymentCommitments, CredentialDeploymentValues,
            CredentialHolderInfo, GlobalContext, IpData, IpIdentity, IpInfo, Policy,
            PublicCredentialData, SignatureThreshold, YearMonth,
        },
    },
    types::{
        AccountIndex, AuthorizationsV0, AuthorizationsV1, BakerCredentials, BakerId, BakerKeyPairs,
        ChainParameterVersion0, ChainParameterVersion1, HigherLevelAccessStructure,
        ProtocolVersion, UpdateKeyPair, UpdateKeysCollection, UpdateKeysCollectionSkeleton,
        UpdatePublicKey,
    },
};
use genesis_creator::{assemble::AssembleGenesisConfig, config::*, genesis::*};
use rayon::prelude::*;
use rust_decimal::prelude::FromPrimitive;
use serde::de::DeserializeOwned;
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::AtomicU64,
};

/// Function for creating the cryptographic parameters (also called global
/// context). The arguments are
/// - `global_out` - if some, where to write the cryptographic parameters
/// - `cfg` - The configuration deciding whether to use existing cryptographic
///   parameters or to generate fresh ones.
///
/// The function returns [`anyhow::Result`], which upon success will contain the
/// cryptographic parameters.
fn crypto_parameters(
    global_out: Option<PathBuf>,
    cfg: CryptoParamsConfig,
) -> anyhow::Result<GlobalContext<ArCurve>> {
    match cfg {
        CryptoParamsConfig::Existing { source } => {
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
        CryptoParamsConfig::Generate { genesis_string } => {
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
            IdentityProviderConfig::Existing { source } => {
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
            IdentityProviderConfig::Fresh { id, repeat } => {
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
            AnonymityRevokerConfig::Existing { source } => {
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
            AnonymityRevokerConfig::Fresh { id, repeat } => {
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
            HigherLevelKey::Existing { source } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the {} key: {}",
                    ctx,
                    source.display()
                ))?;
                let key: UpdatePublicKey = serde_json::from_slice(&data)
                    .context(format!("Could not parse the {} key.", ctx))?;
                out.push(key);
            }
            HigherLevelKey::Fresh { repeat } => {
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
    ensure!(
        !level2_keys.is_empty(),
        "There must be at least one level 2 key.",
    );

    let level2 = update_cfg.level2;
    let emergency = level2.emergency.access_structure(&level2_keys)?;
    let protocol = level2.protocol.access_structure(&level2_keys)?;
    let election_difficulty = level2.election_difficulty.access_structure(&level2_keys)?;
    let euro_per_energy = level2.euro_per_energy.access_structure(&level2_keys)?;
    let micro_gtu_per_euro = level2.micro_ccd_per_euro.access_structure(&level2_keys)?;
    let foundation_account = level2.foundation_account.access_structure(&level2_keys)?;
    let mint_distribution = level2.mint_distribution.access_structure(&level2_keys)?;
    let transaction_fee_distribution = level2
        .transaction_fee_distribution
        .access_structure(&level2_keys)?;
    let param_gas_rewards = level2.gas_rewards.access_structure(&level2_keys)?;
    let pool_parameters = level2.pool_parameters.access_structure(&level2_keys)?;
    let add_anonymity_revoker = level2
        .add_anonymity_revoker
        .access_structure(&level2_keys)?;
    let add_identity_provider = level2
        .add_identity_provider
        .access_structure(&level2_keys)?;

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
    ensure!(
        !level2_keys.is_empty(),
        "There must be at least one level 2 key.",
    );

    let level2 = update_cfg.level2;
    let emergency = level2.emergency.access_structure(&level2_keys)?;
    let protocol = level2.protocol.access_structure(&level2_keys)?;
    let election_difficulty = level2.election_difficulty.access_structure(&level2_keys)?;
    let euro_per_energy = level2.euro_per_energy.access_structure(&level2_keys)?;
    let micro_gtu_per_euro = level2.micro_ccd_per_euro.access_structure(&level2_keys)?;
    let foundation_account = level2.foundation_account.access_structure(&level2_keys)?;
    let mint_distribution = level2.mint_distribution.access_structure(&level2_keys)?;
    let transaction_fee_distribution = level2
        .transaction_fee_distribution
        .access_structure(&level2_keys)?;
    let param_gas_rewards = level2.gas_rewards.access_structure(&level2_keys)?;
    let pool_parameters = level2.pool_parameters.access_structure(&level2_keys)?;
    let add_anonymity_revoker = level2
        .add_anonymity_revoker
        .access_structure(&level2_keys)?;
    let add_identity_provider = level2
        .add_identity_provider
        .access_structure(&level2_keys)?;
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
///
/// The return value is a triple of the index of the foundation account, the
/// number of bakers, and the list of public account data that goes into
/// genesis.
fn accounts(
    baker_keys_out: PathBuf,
    account_keys_out: PathBuf,
    params: &GlobalContext<ArCurve>,
    ars: &BTreeMap<ArIdentity, ArInfo<ArCurve>>,
    cfgs: Vec<AccountConfig>,
) -> anyhow::Result<(AccountIndex, u64, Vec<GenesisAccountPublic>)> {
    let mut foundation_index = None;
    let mut idx: u64 = 0;

    let mut gas = Vec::new();

    let mut csprng = rand::thread_rng();

    let num_bakers = AtomicU64::new(0);

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
                if foundation {
                    let old = foundation_index.replace(AccountIndex::from(idx));
                    if old.is_some() {
                        bail!(
                            "There are two accounts marked as foundation accounts. That will not \
                             work."
                        );
                    }
                }
                let ga: GenesisAccount = serde_json::from_slice(
                    &std::fs::read(source).context("Could not read existing account file.")?,
                )
                .context("Could not parse existing account file.")?;

                let baker = if let Some(stake) = stake {
                    num_bakers.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
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
                                [(CredentialIndex { index: 0 }, acc_cred)]
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
                            num_bakers.fetch_add(1, std::sync::atomic::Ordering::AcqRel);
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
        Ok((
            foundation_index,
            num_bakers.load(std::sync::atomic::Ordering::Acquire),
            gas,
        ))
    } else {
        bail!("Exactly one account must be designated as a foundation account.")
    }
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

/// Check whether the directory exists, and either fail or delete it depending
/// on the value of the `delete_existing` flag.
fn check_and_create_dir(delete_existing: bool, path: &Path, verbose: bool) -> anyhow::Result<()> {
    if path.exists() {
        if delete_existing {
            if verbose {
                println!("Removing existing directory {}", path.display());
            }
            std::fs::remove_dir_all(path).context("Failed to remove the existing directory.")?;
        } else {
            bail!("Supplied output path {} already exists.", path.display());
        }
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

/// Function for assembling the genesis data file given a path to TOML file that
/// can be parsed as a `AssembleGenesisConfig`. Upon success it writes the
/// genesis data to the a file and returns `Ok(())`.
fn handle_assemble(config_path: &Path, verbose: bool) -> anyhow::Result<()> {
    let config_source =
        std::fs::read(config_path).context("Unable to read the configuration file.")?;
    let config: AssembleGenesisConfig =
        toml::from_slice(&config_source).context("Unable to parse the configuration file.")?;
    let accounts: Vec<GenesisAccountPublic> =
        read_json(&make_relative(config_path, &config.accounts)?)?;
    let global = read_json::<Versioned<_>>(&make_relative(config_path, &config.global)?)?;
    let idps = read_json::<Versioned<_>>(&make_relative(config_path, &config.idps)?)?;
    let ars = read_json::<Versioned<_>>(&make_relative(config_path, &config.ars)?)?;

    if verbose {
        println!("Using the following configuration structure for generating genesis.");
        println!("{:#?}", config);
    }

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

    println!(
        "The genesis data will be stored in {}",
        config.genesis_out.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.genesis_hash_out.display()
    );

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

            let genesis_hash = genesis.hash();
            std::fs::write(
                config.genesis_hash_out,
                serde_json::to_vec_pretty(&[genesis_hash])
                    .expect("JSON serialization of hashes should not fail."),
            )
            .context("Unable to write the genesis hash.")?;
        }
    }
    println!("DONE");
    Ok(())
}

/// Function for generating a new genesis data file given a path to TOML file
/// that can be parsed as a `Config`. Upon success it writes the genesis data to
/// the a file and writes all relevant private data to their desired locations
/// and returns `Ok(())`.
fn handle_generate(config_path: &Path, verbose: bool) -> anyhow::Result<()> {
    let config_source =
        std::fs::read(config_path).context("Unable to read the configuration file.")?;
    let config: Config =
        toml::from_slice(&config_source).context("Unable to parse the configuration file.")?;
    if verbose {
        println!("Using the following configuration structure for generating genesis.");
        println!("{:#?}", config);
    }

    if config.out.delete_existing {
        println!("Deleting any existing directories.")
    }

    check_and_create_dir(
        config.out.delete_existing,
        &config.out.account_keys,
        verbose,
    )?;
    check_and_create_dir(config.out.delete_existing, &config.out.update_keys, verbose)?;
    check_and_create_dir(
        config.out.delete_existing,
        &config.out.identity_providers,
        verbose,
    )?;
    check_and_create_dir(
        config.out.delete_existing,
        &config.out.anonymity_revokers,
        verbose,
    )?;
    check_and_create_dir(config.out.delete_existing, &config.out.baker_keys, verbose)?;
    if let Some(global) = &config.out.cryptographic_parameters {
        check_and_create_dir(config.out.delete_existing, global, verbose)?;
    }

    println!(
        "Account keys will be generated in {}",
        config.out.account_keys.display()
    );
    println!(
        "Chain update keys will be generated in {}",
        config.out.update_keys.display()
    );
    println!(
        "Identity providers will be generated in {}",
        config.out.identity_providers.display()
    );
    println!(
        "Anonymity revokers will be generated in {}",
        config.out.anonymity_revokers.display()
    );
    println!(
        "Baker keys will be generated in {}",
        config.out.baker_keys.display()
    );
    if let Some(global) = &config.out.cryptographic_parameters {
        println!(
            "Cryptographic parameter will be generated in {}",
            global.display()
        );
    }

    println!(
        "The genesis data will be stored in {}",
        config.out.genesis.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.out.genesis_hash.display()
    );

    let core = config.parameters.to_core()?;

    let genesis_time = chrono::DateTime::<chrono::Utc>::from(std::time::UNIX_EPOCH)
        + chrono::Duration::milliseconds(core.time.millis as i64);

    println!("Genesis time is set to {}.", genesis_time);
    let slot_duration = rust_decimal::Decimal::from_u64(config.parameters.slot_duration.millis)
        .context("Too large slot duration.")?;
    let elect_diff = config.parameters.chain.election_difficulty();
    let average_block_time: rust_decimal::Decimal =
        slot_duration / rust_decimal::Decimal::from(elect_diff);
    println!("Average block time is set to {}ms.", average_block_time);

    let cryptographic_parameters = crypto_parameters(
        config.out.cryptographic_parameters,
        config.cryptographic_parameters,
    )?;
    let identity_providers =
        identity_providers(config.out.identity_providers, config.identity_providers)?;
    let anonymity_revokers = anonymity_revokers(
        config.out.anonymity_revokers,
        &cryptographic_parameters,
        config.anonymity_revokers,
    )?;
    let (foundation_idx, num_bakers, accounts) = accounts(
        config.out.baker_keys,
        config.out.account_keys,
        &cryptographic_parameters,
        &anonymity_revokers,
        config.accounts,
    )?;

    println!(
        "There are {} accounts in genesis, {} of which are bakers.",
        accounts.len(),
        num_bakers
    );

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
        ProtocolVersion::P4 | ProtocolVersion::P5 => {
            let core = config.parameters.to_core()?;
            let params = match config.parameters.chain {
                GenesisChainParameters::V1(params) => params,
                GenesisChainParameters::V0(_) => {
                    bail!(format!(
                        "Protocol version P4 supports only chain parameters version 1."
                    ))
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
            match config.protocol_version {
                ProtocolVersion::P1 | ProtocolVersion::P2 | ProtocolVersion::P3 => {
                    unreachable!("Already checked.")
                }
                ProtocolVersion::P4 => GenesisData::P4 {
                    core,
                    initial_state,
                },
                ProtocolVersion::P5 => GenesisData::P5 {
                    core,
                    initial_state,
                },
            }
        }
    };
    {
        let mut out = Vec::new();
        genesis.serial(&mut out);
        std::fs::write(config.out.genesis, out).context("Unable to write genesis.")?;

        let genesis_hash = genesis.hash();
        std::fs::write(
            config.out.genesis_hash,
            serde_json::to_vec_pretty(&[genesis_hash])
                .expect("JSON serialization of hashes should not fail."),
        )
        .context("Unable to write the genesis hash.")?;
    }
    println!("DONE");
    Ok(())
}

/// Subcommands supported by the tool.
#[derive(clap::Subcommand, Debug)]
#[clap(author, version, about)]
enum GenesisCreatorCommand {
    Assemble {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config:  PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
    Generate {
        #[clap(long, short)]
        /// The TOML configuration file describing the genesis.
        config:  PathBuf,
        #[clap(long, short)]
        /// Whether to output additional data during genesis generation.
        verbose: bool,
    },
}

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct GenesisCreator {
    #[clap(subcommand)]
    action: GenesisCreatorCommand,
}

fn main() -> anyhow::Result<()> {
    let args = GenesisCreator::parse();

    match &args.action {
        GenesisCreatorCommand::Assemble { config, verbose } => handle_assemble(config, *verbose),
        GenesisCreatorCommand::Generate { config, verbose } => handle_generate(config, *verbose),
    }
}

// TODO: Deny unused fields.
// TODO: Output genesis_hash

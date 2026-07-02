pub(crate) mod assemble;
pub(crate) mod config;
pub(crate) mod genesis;

/// A command line tool for generating genesis files.
///
/// The tool has two modes: `generate` that can generate a new genesis,
/// potentially reusing some files/keys from the previously generated genesis,
/// and `assemble` that can produce a genesis from existing files (for example
/// to regenereate the Mainnet `genesis.dat`).
///
/// In both modes the tool takes a TOML configuration file that specifies the
/// genesis. For details, see the README.
use crate::{assemble::AssembleGenesisConfig, config::*, genesis::*};
use anyhow::{anyhow, Context};
use concordium_rust_sdk::{
    common::{Versioned, VERSION_0},
    genesis::{
        builder::GenesisBuilderCommon,
        builder::{
            FreshAccountConfig, GovernanceKeyLevelConfig, GovernanceKeySpec,
            GovernanceKeysGenerateConfig, GovernanceKeysInput, Level2AccessConfig,
            Level2GovernanceKeysConfig,
        },
        genesis_builder_p1, genesis_builder_p10, genesis_builder_p11, genesis_builder_p2,
        genesis_builder_p3, genesis_builder_p4, genesis_builder_p5, genesis_builder_p6,
        genesis_builder_p7, genesis_builder_p8, genesis_builder_p9,
        output::GenesisOutputCPV,
    },
    id::{
        constants::{ArCurve, IpPairing},
        types::{ArIdentity, ArInfo, GlobalContext, IpIdentity, IpInfo, SignatureThreshold},
    },
    types::{
        AuthorizationsV0, AuthorizationsV1, BakerCredentials, ProtocolVersion, UpdatePublicKey,
    },
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

fn read_json<S: serde::de::DeserializeOwned>(path: &Path) -> anyhow::Result<S> {
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
            anyhow::bail!("Supplied output path {} already exists.", path.display());
        }
    }
    std::fs::create_dir_all(path)?;
    Ok(())
}

// ── Shared helpers ────────────────────────────────────────────────────────────

/// Tracks a freshly generated account so its key file can be named correctly
/// after [`populate_builder`] returns.
struct FreshAccountEntry {
    /// Index of this account in `GenesisOutput::account_data`.
    account_data_index: usize,
    /// Global account number (used in the output file name).
    global_index: u64,
    /// File-name template (e.g. `"baker"` or `"account"`).
    template: String,
}

/// Create all output directories declared in `out` and print the corresponding
/// announcements.  Called once at the top of every `handle_generate_cpvN`.
fn prepare_output_directories(out: &OutputConfig, verbose: bool) -> anyhow::Result<()> {
    if out.delete_existing {
        println!("Deleting any existing directories.");
    }
    check_and_create_dir(out.delete_existing, &out.account_keys, verbose)?;
    if let Some(dir) = out.update_keys.as_ref() {
        check_and_create_dir(out.delete_existing, dir, verbose)?;
    }
    check_and_create_dir(out.delete_existing, &out.identity_providers, verbose)?;
    check_and_create_dir(out.delete_existing, &out.anonymity_revokers, verbose)?;
    check_and_create_dir(out.delete_existing, &out.baker_keys, verbose)?;
    if let Some(global) = &out.cryptographic_parameters {
        check_and_create_dir(out.delete_existing, global, verbose)?;
    }

    println!(
        "Account keys will be generated in {}",
        out.account_keys.display()
    );
    if let Some(dir) = out.update_keys.as_ref() {
        println!("Chain update keys will be generated in {}", dir.display());
    }
    println!(
        "Identity providers will be generated in {}",
        out.identity_providers.display()
    );
    println!(
        "Anonymity revokers will be generated in {}",
        out.anonymity_revokers.display()
    );
    println!(
        "Baker keys will be generated in {}",
        out.baker_keys.display()
    );
    if let Some(global) = &out.cryptographic_parameters {
        println!(
            "Cryptographic parameters will be generated in {}",
            global.display()
        );
    }
    println!(
        "The genesis data will be stored in {}",
        out.genesis.display()
    );
    println!(
        "The genesis hash will be written to {}",
        out.genesis_hash.display()
    );
    Ok(())
}

/// Feed cryptographic parameters, identity providers, anonymity revokers, and
/// accounts from `config` into `builder`.
///
/// Returns the populated builder together with a `fresh_entries` list that
/// maps each freshly generated account's position in `output.account_data`
/// to its global index and file-name template — used by `write_generate_output`
/// to name the per-account key files.
fn populate_builder<B: GenesisBuilderCommon>(
    mut builder: B,
    crypto_params_cfg: CryptoParamsConfig,
    identity_providers: Vec<IdentityProviderConfig>,
    anonymity_revokers: Vec<AnonymityRevokerConfig>,
    accounts: Vec<AccountConfig>,
) -> anyhow::Result<(B, Vec<FreshAccountEntry>)> {
    // Crypto params
    builder = match crypto_params_cfg {
        CryptoParamsConfig::Existing { source } => {
            let data = std::fs::read(&source).context(format!(
                "Could not read cryptographic parameters: {}",
                source.display()
            ))?;
            let ver: Versioned<GlobalContext<ArCurve>> = serde_json::from_slice(&data)
                .context("Could not parse cryptographic parameters.")?;
            anyhow::ensure!(
                ver.version == 0.into(),
                "Incorrect version of cryptographic parameters."
            );
            builder.with_crypto_params(ver.value)
        }
        CryptoParamsConfig::Generate { genesis_string } => {
            builder.generate_crypto_params(genesis_string)
        }
    };

    // Identity providers
    for ip_cfg in identity_providers {
        builder = match ip_cfg {
            IdentityProviderConfig::Existing { source } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the identity provider file: {}",
                    source.display()
                ))?;
                let ver: Versioned<IpInfo<IpPairing>> =
                    serde_json::from_slice(&data).context("Could not parse identity provider.")?;
                builder.add_identity_provider_public(ver.value)
            }
            IdentityProviderConfig::Fresh { id, repeat } => {
                builder.generate_identity_providers(id, repeat.unwrap_or(1))
            }
        };
    }

    // Anonymity revokers
    for ar_cfg in anonymity_revokers {
        builder = match ar_cfg {
            AnonymityRevokerConfig::Existing { source } => {
                let data = std::fs::read(&source).context(format!(
                    "Could not read the anonymity revoker file: {}",
                    source.display()
                ))?;
                let ver: Versioned<ArInfo<ArCurve>> =
                    serde_json::from_slice(&data).context("Could not parse anonymity revoker.")?;
                builder.add_anonymity_revoker_public(ver.value)
            }
            AnonymityRevokerConfig::Fresh { id, repeat } => {
                builder.generate_anonymity_revokers(id, repeat.unwrap_or(1))
            }
        };
    }

    // Accounts
    let mut fresh_entries: Vec<FreshAccountEntry> = Vec::new();
    let mut running_idx: u64 = 0;
    let mut ad_count: usize = 0;

    for acc_cfg in accounts {
        builder = match acc_cfg {
            AccountConfig::Existing {
                source,
                foundation,
                balance,
                stake,
                restake_earnings,
                baker_keys,
            } => {
                let account: crate::genesis::GenesisAccount =
                    serde_json::from_slice(&std::fs::read(&source).context(format!(
                        "Could not read existing account file: {}",
                        source.display()
                    ))?)
                    .context("Could not parse existing account file.")?;
                let baker_creds = if let Some(bk) = baker_keys {
                    let creds: BakerCredentials =
                        serde_json::from_slice(&std::fs::read(&bk).context(format!(
                            "Could not read baker credentials file: {}",
                            bk.display()
                        ))?)
                        .context("Could not parse baker credentials file.")?;
                    Some(creds)
                } else {
                    None
                };
                running_idx += 1;
                ad_count += 1;
                builder.add_existing_account(
                    account,
                    balance,
                    stake,
                    restake_earnings,
                    baker_creds,
                    foundation,
                )
            }
            AccountConfig::Fresh {
                repeat,
                stake,
                balance,
                template,
                identity_provider,
                num_keys,
                threshold,
                restake_earnings,
                foundation,
            } => {
                let count = u64::from(repeat.unwrap_or(1));
                for (offset, n) in (running_idx..(running_idx + count)).enumerate() {
                    fresh_entries.push(FreshAccountEntry {
                        account_data_index: ad_count + offset,
                        global_index: n,
                        template: template.clone(),
                    });
                }
                ad_count += count as usize;
                running_idx += count;
                builder.generate_accounts(FreshAccountConfig {
                    count: count as u32,
                    stake,
                    balance,
                    num_keys: num_keys.unwrap_or(1),
                    threshold: threshold.unwrap_or(SignatureThreshold::ONE),
                    identity_provider,
                    restake_earnings,
                    foundation,
                })
            }
        };
    }

    Ok((builder, fresh_entries))
}

/// Artifacts shared across all assemble handlers.
struct AssembleArtifacts {
    accounts: Vec<GenesisAccountPublic>,
    global: Versioned<GlobalContext<ArCurve>>,
    idps: Versioned<BTreeMap<IpIdentity, IpInfo<IpPairing>>>,
    ars: Versioned<BTreeMap<ArIdentity, ArInfo<ArCurve>>>,
}

/// Read the four JSON artifact files that every assemble handler needs.
fn load_assemble_artifacts(
    config_path: &Path,
    config: &AssembleGenesisConfig,
) -> anyhow::Result<AssembleArtifacts> {
    Ok(AssembleArtifacts {
        accounts: read_json(&make_relative(config_path, &config.accounts)?)?,
        global: read_json(&make_relative(config_path, &config.global)?)?,
        idps: read_json(&make_relative(config_path, &config.idps)?)?,
        ars: read_json(&make_relative(config_path, &config.ars)?)?,
    })
}

/// Feed assemble-mode artifacts into a builder: crypto params, IPs, ARs, and
/// existing public accounts.
fn populate_assemble_builder<B: GenesisBuilderCommon>(
    mut builder: B,
    artifacts: AssembleArtifacts,
    foundation_addr: concordium_rust_sdk::id::types::AccountAddress,
) -> anyhow::Result<B> {
    anyhow::ensure!(
        artifacts
            .accounts
            .iter()
            .any(|a| a.address == foundation_addr),
        "Cannot find foundation account."
    );
    builder = builder.with_crypto_params(artifacts.global.value);
    for (_, ip_info) in artifacts.idps.value {
        builder = builder.add_identity_provider_public(ip_info);
    }
    for (_, ar_info) in artifacts.ars.value {
        builder = builder.add_anonymity_revoker_public(ar_info);
    }
    for account in artifacts.accounts {
        let is_foundation = account.address == foundation_addr;
        builder = builder.add_existing_public_account(account, is_foundation);
    }
    Ok(builder)
}

// ── Top-level entry points ────────────────────────────────────────────────────

pub fn handle_assemble(config_path: &Path, verbose: bool) -> anyhow::Result<()> {
    let config_source =
        std::fs::read(config_path).context("Unable to read the configuration file.")?;
    let config: AssembleGenesisConfig =
        toml::from_slice(&config_source).context("Unable to parse the configuration file.")?;
    if verbose {
        println!("Using the following configuration structure for generating genesis.");
        println!("{config:#?}");
    }
    match config.protocol.protocol_version() {
        ProtocolVersion::P1 | ProtocolVersion::P2 | ProtocolVersion::P3 => {
            handle_assemble_cpv0(config_path, config)
        }
        ProtocolVersion::P4 | ProtocolVersion::P5 => handle_assemble_cpv1(config_path, config),
        ProtocolVersion::P6 | ProtocolVersion::P7 => handle_assemble_cpv2(config_path, config),
        ProtocolVersion::P8 | ProtocolVersion::P9 | ProtocolVersion::P10 | ProtocolVersion::P11 => {
            handle_assemble_cpv3(config_path, config)
        }
    }
}

pub fn handle_generate(config_path: &Path, verbose: bool) -> anyhow::Result<()> {
    let config_source =
        std::fs::read(config_path).context("Unable to read the configuration file.")?;
    let config: Config =
        toml::from_slice(&config_source).context("Unable to parse the configuration file.")?;
    if verbose {
        println!("Using the following configuration structure for generating genesis.");
        println!("{config:#?}");
    }
    match config.protocol.protocol_version() {
        ProtocolVersion::P1 | ProtocolVersion::P2 | ProtocolVersion::P3 => {
            handle_generate_cpv0(config, verbose)
        }
        ProtocolVersion::P4 | ProtocolVersion::P5 => handle_generate_cpv1(config, verbose),
        ProtocolVersion::P6 | ProtocolVersion::P7 => handle_generate_cpv2(config, verbose),
        ProtocolVersion::P8 | ProtocolVersion::P9 | ProtocolVersion::P10 | ProtocolVersion::P11 => {
            handle_generate_cpv3(config, verbose)
        }
    }
}

// ── Output writing ────────────────────────────────────────────────────────────

fn write_generate_output<GK: serde::Serialize>(
    out_cfg: &OutputConfig,
    fresh_entries: &[FreshAccountEntry],
    output: &GenesisOutputCPV<GK>,
) -> anyhow::Result<()> {
    // Crypto params
    if let Some(global_out) = &out_cfg.cryptographic_parameters {
        let ver = Versioned {
            version: VERSION_0,
            value: &output.crypto_params,
        };
        let mut path = global_out.clone();
        path.push("cryptographic-parameters.json");
        std::fs::write(path, serde_json::to_string_pretty(&ver).unwrap())
            .context("Unable to write cryptographic parameters.")?;
    }

    // Identity providers
    let idp_out = &out_cfg.identity_providers;
    for ip_data in &output.identity_provider_data {
        let n = ip_data.public_ip_info.ip_identity.0;
        let mut path = idp_out.clone();
        path.push(format!("ip-data-{n}.json"));
        std::fs::write(path, serde_json::to_string_pretty(ip_data).unwrap())
            .context("Unable to write identity provider.")?;
    }
    {
        let ver_idps = Versioned {
            version: VERSION_0,
            value: &output.identity_provider_infos,
        };
        let mut path = idp_out.clone();
        path.push("identity-providers.json");
        std::fs::write(path, serde_json::to_string_pretty(&ver_idps).unwrap())
            .context("Unable to write identity providers.")?;
    }

    // Anonymity revokers
    let ars_out = &out_cfg.anonymity_revokers;
    for ar_data in &output.anonymity_revoker_data {
        let n = u32::from(ar_data.public_ar_info.ar_identity);
        let mut path = ars_out.clone();
        path.push(format!("ar-data-{n}.json"));
        std::fs::write(path, serde_json::to_string_pretty(ar_data).unwrap())
            .context("Unable to write anonymity revoker.")?;
    }
    {
        let ver_ars = Versioned {
            version: VERSION_0,
            value: &output.anonymity_revoker_infos,
        };
        let mut path = ars_out.clone();
        path.push("anonymity-revokers.json");
        std::fs::write(path, serde_json::to_string_pretty(&ver_ars).unwrap())
            .context("Unable to write anonymity revokers.")?;
    }

    // Freshly generated accounts
    for entry in fresh_entries {
        let ga = &output.account_data[entry.account_data_index];
        let path = out_cfg
            .account_keys
            .join(format!("{}-{}.json", entry.template, entry.global_index));
        std::fs::write(&path, serde_json::to_string_pretty(ga).unwrap())
            .context(format!("Unable to write account {}.", entry.global_index))?;
    }
    {
        let path = out_cfg.account_keys.join("accounts.json");
        std::fs::write(
            path,
            serde_json::to_string_pretty(&output.accounts_public).unwrap(),
        )
        .context("Unable to write accounts.")?;
    }

    // Baker credentials
    {
        let mut baker_creds_iter = output.baker_credentials.iter();
        for (i, acc_public) in output.accounts_public.iter().enumerate() {
            if acc_public.baker.is_some() {
                if let Some(creds) = baker_creds_iter.next() {
                    let path = out_cfg
                        .baker_keys
                        .join(format!("baker-{i}-credentials.json"));
                    std::fs::write(path, serde_json::to_string_pretty(creds).unwrap()).context(
                        format!("Unable to write baker credentials for account {}.", i),
                    )?;
                }
            }
        }
    }

    // Governance keys and generated key pairs
    if let Some(keys_out) = &out_cfg.update_keys {
        for gkp in &output.generated_root_key_pairs {
            let path = keys_out.join(format!("root-key-{}.json", gkp.index));
            std::fs::write(path, serde_json::to_string_pretty(&gkp.key_pair).unwrap())
                .context("Unable to write root key.")?;
        }
        for gkp in &output.generated_level1_key_pairs {
            let path = keys_out.join(format!("level1-key-{}.json", gkp.index));
            std::fs::write(path, serde_json::to_string_pretty(&gkp.key_pair).unwrap())
                .context("Unable to write level-1 key.")?;
        }
        for gkp in &output.generated_level2_key_pairs {
            let path = keys_out.join(format!("level2-key-{}.json", gkp.index));
            std::fs::write(path, serde_json::to_string_pretty(&gkp.key_pair).unwrap())
                .context("Unable to write level-2 key.")?;
        }
        let governance_path = keys_out.join("governance-keys.json");
        std::fs::write(
            governance_path,
            serde_json::to_string_pretty(&output.governance_keys).unwrap(),
        )
        .context("Unable to write governance keys.")?;
    }

    // Genesis block
    write_genesis(
        out_cfg.genesis.as_path(),
        out_cfg.genesis_hash.as_path(),
        &output.genesis_data,
    )?;
    println!("DONE");
    Ok(())
}

fn write_genesis(data_path: &Path, hash_path: &Path, genesis: &GenesisData) -> anyhow::Result<()> {
    std::fs::write(
        data_path,
        concordium_rust_sdk::genesis::serialize_genesis(genesis),
    )
    .context("Unable to write genesis.")?;
    std::fs::write(
        hash_path,
        serde_json::to_vec_pretty(&[genesis.hash()])
            .expect("JSON serialization of hashes should not fail."),
    )
    .context("Unable to write the genesis hash.")?;
    Ok(())
}

// ── CPV3 handlers (P8+) ────────────────────────────────────────────────────

fn handle_assemble_cpv3(config_path: &Path, config: AssembleGenesisConfig) -> anyhow::Result<()> {
    let artifacts = load_assemble_artifacts(config_path, &config)?;
    println!(
        "The genesis data will be stored in {}",
        config.genesis_out.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.genesis_hash_out.display()
    );

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = match config.protocol {
        ProtocolConfigToml::P8 { parameters } => (
            genesis_builder_p8(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV3::try_from(parameters)?,
        ),
        ProtocolConfigToml::P9 { parameters } => (
            genesis_builder_p9(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV3::try_from(parameters)?,
        ),
        ProtocolConfigToml::P10 { parameters } => (
            genesis_builder_p10(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV3::try_from(parameters)?,
        ),
        ProtocolConfigToml::P11 { parameters } => (
            genesis_builder_p11(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV3::try_from(parameters)?,
        ),
        _ => unreachable!("handle_assemble_cpv3 only receives P8–P11"),
    };
    let governance_keys: UpdateKeysCollectionSkeleton<AuthorizationsV1> =
        read_json(&make_relative(config_path, &config.governance_keys)?)?;
    let builder = populate_assemble_builder(builder, artifacts, config.foundation_account)?
        .with_governance_keys(governance_keys)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    write_genesis(
        &make_relative(config_path, &config.genesis_out)?,
        &make_relative(config_path, &config.genesis_hash_out)?,
        &output.genesis_data,
    )?;
    println!("DONE");
    Ok(())
}

fn handle_generate_cpv3(config: Config, verbose: bool) -> anyhow::Result<()> {
    prepare_output_directories(&config.out, verbose)?;

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = {
        use concordium_rust_sdk::genesis::ProtocolParamsCPV3;
        match config.protocol {
            ProtocolConfigToml::P8 { parameters } => (
                genesis_builder_p8(),
                ProtocolParamsCPV3::try_from(parameters)?,
            ),
            ProtocolConfigToml::P9 { parameters } => (
                genesis_builder_p9(),
                ProtocolParamsCPV3::try_from(parameters)?,
            ),
            ProtocolConfigToml::P10 { parameters } => (
                genesis_builder_p10(),
                ProtocolParamsCPV3::try_from(parameters)?,
            ),
            ProtocolConfigToml::P11 { parameters } => (
                genesis_builder_p11(),
                ProtocolParamsCPV3::try_from(parameters)?,
            ),
            _ => unreachable!("handle_generate_cpv3 only receives P8–P11"),
        }
    };
    let (builder, fresh_entries) = populate_builder(
        builder,
        config.cryptographic_parameters,
        config.identity_providers,
        config.anonymity_revokers,
        config.accounts,
    )?;
    let builder = builder
        .with_governance_keys_input(convert_update_keys_config(&config.updates)?)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    println!(
        "There are {} accounts in genesis, {} of which are bakers.",
        output.accounts_public.len(),
        output.baker_credentials.len()
    );
    write_generate_output(&config.out, &fresh_entries, &output)
}

// ── CPV2 handlers (P6–P7) ────────────────────────────────────────────────────

fn handle_assemble_cpv2(config_path: &Path, config: AssembleGenesisConfig) -> anyhow::Result<()> {
    let artifacts = load_assemble_artifacts(config_path, &config)?;
    println!(
        "The genesis data will be stored in {}",
        config.genesis_out.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.genesis_hash_out.display()
    );

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = match config.protocol {
        ProtocolConfigToml::P6 { parameters } => (
            genesis_builder_p6(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV2::try_from(parameters)?,
        ),
        ProtocolConfigToml::P7 { parameters } => (
            genesis_builder_p7(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV2::try_from(parameters)?,
        ),
        _ => unreachable!("handle_assemble_cpv2 only receives P6/P7"),
    };
    let governance_keys: UpdateKeysCollectionSkeleton<AuthorizationsV1> =
        read_json(&make_relative(config_path, &config.governance_keys)?)?;
    let builder = populate_assemble_builder(builder, artifacts, config.foundation_account)?
        .with_governance_keys(governance_keys)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    write_genesis(
        &make_relative(config_path, &config.genesis_out)?,
        &make_relative(config_path, &config.genesis_hash_out)?,
        &output.genesis_data,
    )?;
    println!("DONE");
    Ok(())
}

fn handle_generate_cpv2(config: Config, verbose: bool) -> anyhow::Result<()> {
    prepare_output_directories(&config.out, verbose)?;

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = {
        use concordium_rust_sdk::genesis::ProtocolParamsCPV2;
        match config.protocol {
            ProtocolConfigToml::P6 { parameters } => (
                genesis_builder_p6(),
                ProtocolParamsCPV2::try_from(parameters)?,
            ),
            ProtocolConfigToml::P7 { parameters } => (
                genesis_builder_p7(),
                ProtocolParamsCPV2::try_from(parameters)?,
            ),
            _ => unreachable!("handle_generate_cpv2 only receives P6/P7"),
        }
    };
    let (builder, fresh_entries) = populate_builder(
        builder,
        config.cryptographic_parameters,
        config.identity_providers,
        config.anonymity_revokers,
        config.accounts,
    )?;
    let builder = builder
        .with_governance_keys_input(convert_update_keys_config(&config.updates)?)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    println!(
        "There are {} accounts in genesis, {} of which are bakers.",
        output.accounts_public.len(),
        output.baker_credentials.len()
    );
    write_generate_output(&config.out, &fresh_entries, &output)
}

// ── CPV1 handlers (P4–P5) ────────────────────────────────────────────────────

fn handle_assemble_cpv1(config_path: &Path, config: AssembleGenesisConfig) -> anyhow::Result<()> {
    let artifacts = load_assemble_artifacts(config_path, &config)?;
    println!(
        "The genesis data will be stored in {}",
        config.genesis_out.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.genesis_hash_out.display()
    );

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = match config.protocol {
        ProtocolConfigToml::P4 { parameters } => (
            genesis_builder_p4(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV1::try_from(parameters)?,
        ),
        ProtocolConfigToml::P5 { parameters } => (
            genesis_builder_p5(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV1::try_from(parameters)?,
        ),
        _ => unreachable!("handle_assemble_cpv1 only receives P4/P5"),
    };
    let governance_keys: UpdateKeysCollectionSkeleton<AuthorizationsV1> =
        read_json(&make_relative(config_path, &config.governance_keys)?)?;
    let builder = populate_assemble_builder(builder, artifacts, config.foundation_account)?
        .with_governance_keys(governance_keys)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    write_genesis(
        &make_relative(config_path, &config.genesis_out)?,
        &make_relative(config_path, &config.genesis_hash_out)?,
        &output.genesis_data,
    )?;
    println!("DONE");
    Ok(())
}

fn handle_generate_cpv1(config: Config, verbose: bool) -> anyhow::Result<()> {
    prepare_output_directories(&config.out, verbose)?;

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = {
        use concordium_rust_sdk::genesis::ProtocolParamsCPV1;
        match config.protocol {
            ProtocolConfigToml::P4 { parameters } => (
                genesis_builder_p4(),
                ProtocolParamsCPV1::try_from(parameters)?,
            ),
            ProtocolConfigToml::P5 { parameters } => (
                genesis_builder_p5(),
                ProtocolParamsCPV1::try_from(parameters)?,
            ),
            _ => unreachable!("handle_generate_cpv1 only receives P4/P5"),
        }
    };
    let (builder, fresh_entries) = populate_builder(
        builder,
        config.cryptographic_parameters,
        config.identity_providers,
        config.anonymity_revokers,
        config.accounts,
    )?;
    let builder = builder
        .with_governance_keys_input(convert_update_keys_config(&config.updates)?)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    println!(
        "There are {} accounts in genesis, {} of which are bakers.",
        output.accounts_public.len(),
        output.baker_credentials.len()
    );
    write_generate_output(&config.out, &fresh_entries, &output)
}

// ── CPV0 handlers (P1–P3) ────────────────────────────────────────────────────

fn handle_assemble_cpv0(config_path: &Path, config: AssembleGenesisConfig) -> anyhow::Result<()> {
    let artifacts = load_assemble_artifacts(config_path, &config)?;
    println!(
        "The genesis data will be stored in {}",
        config.genesis_out.display()
    );
    println!(
        "The genesis hash will be written to {}",
        config.genesis_hash_out.display()
    );

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = match config.protocol {
        ProtocolConfigToml::P1 { parameters } => (
            genesis_builder_p1(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV0::try_from(parameters)?,
        ),
        ProtocolConfigToml::P2 { parameters } => (
            genesis_builder_p2(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV0::try_from(parameters)?,
        ),
        ProtocolConfigToml::P3 { parameters } => (
            genesis_builder_p3(),
            concordium_rust_sdk::genesis::ProtocolParamsCPV0::try_from(parameters)?,
        ),
        _ => unreachable!("handle_assemble_cpv0 only receives P1/P2/P3"),
    };
    let governance_keys: UpdateKeysCollectionSkeleton<AuthorizationsV0> =
        read_json(&make_relative(config_path, &config.governance_keys)?)?;
    let builder = populate_assemble_builder(builder, artifacts, config.foundation_account)?
        .with_governance_keys(governance_keys)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    write_genesis(
        &make_relative(config_path, &config.genesis_out)?,
        &make_relative(config_path, &config.genesis_hash_out)?,
        &output.genesis_data,
    )?;
    println!("DONE");
    Ok(())
}

fn handle_generate_cpv0(config: Config, verbose: bool) -> anyhow::Result<()> {
    prepare_output_directories(&config.out, verbose)?;

    use crate::config::ProtocolConfigToml;
    let (builder, protocol_params) = {
        use concordium_rust_sdk::genesis::ProtocolParamsCPV0;
        match config.protocol {
            ProtocolConfigToml::P1 { parameters } => (
                genesis_builder_p1(),
                ProtocolParamsCPV0::try_from(parameters)?,
            ),
            ProtocolConfigToml::P2 { parameters } => (
                genesis_builder_p2(),
                ProtocolParamsCPV0::try_from(parameters)?,
            ),
            ProtocolConfigToml::P3 { parameters } => (
                genesis_builder_p3(),
                ProtocolParamsCPV0::try_from(parameters)?,
            ),
            _ => unreachable!("handle_generate_cpv0 only receives P1/P2/P3"),
        }
    };
    let (builder, fresh_entries) = populate_builder(
        builder,
        config.cryptographic_parameters,
        config.identity_providers,
        config.anonymity_revokers,
        config.accounts,
    )?;
    let builder = builder
        .with_governance_keys_input(convert_update_keys_config_v0(&config.updates)?)
        .with_protocol(protocol_params);
    let output = builder.build()?;
    println!(
        "There are {} accounts in genesis, {} of which are bakers.",
        output.accounts_public.len(),
        output.baker_credentials.len()
    );
    write_generate_output(&config.out, &fresh_entries, &output)
}

// ── Governance key converters ─────────────────────────────────────────────────

/// Convert `UpdateKeysConfig` (CLI, path-based) to the library's
/// `GovernanceKeysInput` (in-memory).  Reads public key files from disk.
fn convert_update_keys_config(cfg: &UpdateKeysConfig) -> anyhow::Result<GovernanceKeysInput> {
    let root_specs = convert_key_specs(&cfg.root.keys)?;
    let level1_specs = convert_key_specs(&cfg.level1.keys)?;
    let level2_specs = convert_key_specs(&cfg.level2.keys)?;
    let l2 = &cfg.level2;
    Ok(GovernanceKeysInput::Generate(Box::new(
        GovernanceKeysGenerateConfig {
            root: GovernanceKeyLevelConfig {
                threshold: cfg.root.threshold,
                keys: root_specs,
            },
            level1: GovernanceKeyLevelConfig {
                threshold: cfg.level1.threshold,
                keys: level1_specs,
            },
            level2: Level2GovernanceKeysConfig {
                keys: level2_specs,
                emergency: level2_access(&l2.emergency),
                protocol: level2_access(&l2.protocol),
                election_difficulty: level2_access(&l2.election_difficulty),
                euro_per_energy: level2_access(&l2.euro_per_energy),
                micro_ccd_per_euro: level2_access(&l2.micro_ccd_per_euro),
                foundation_account: level2_access(&l2.foundation_account),
                mint_distribution: level2_access(&l2.mint_distribution),
                transaction_fee_distribution: level2_access(&l2.transaction_fee_distribution),
                gas_rewards: level2_access(&l2.gas_rewards),
                pool_parameters: level2_access(&l2.pool_parameters),
                add_anonymity_revoker: level2_access(&l2.add_anonymity_revoker),
                add_identity_provider: level2_access(&l2.add_identity_provider),
                cooldown_parameters: level2_access(l2.cooldown_parameters.as_ref().ok_or_else(
                    || anyhow!("cooldownParameters missing from governance key config"),
                )?),
                time_parameters: level2_access(
                    l2.time_parameters.as_ref().ok_or_else(|| {
                        anyhow!("timeParameters missing from governance key config")
                    })?,
                ),
                create_plt: l2.create_plt.as_ref().map(level2_access),
            },
        },
    )))
}

/// Convert a CPV0 `UpdateKeysConfig` to the library's `GovernanceKeysInputCPV0`.
///
/// CPV0 does not have cooldown_parameters, time_parameters, or create_plt.
fn convert_update_keys_config_v0(
    cfg: &UpdateKeysConfig,
) -> anyhow::Result<concordium_rust_sdk::genesis::GovernanceKeysInputCPV0> {
    use concordium_rust_sdk::genesis::{
        GovernanceKeysGenerateConfigCPV0, GovernanceKeysInputCPV0, Level2GovernanceKeysConfigV0,
    };
    let root_specs = convert_key_specs(&cfg.root.keys)?;
    let level1_specs = convert_key_specs(&cfg.level1.keys)?;
    let level2_specs = convert_key_specs(&cfg.level2.keys)?;
    let l2 = &cfg.level2;
    Ok(GovernanceKeysInputCPV0::Generate(Box::new(
        GovernanceKeysGenerateConfigCPV0 {
            root: GovernanceKeyLevelConfig {
                threshold: cfg.root.threshold,
                keys: root_specs,
            },
            level1: GovernanceKeyLevelConfig {
                threshold: cfg.level1.threshold,
                keys: level1_specs,
            },
            level2: Level2GovernanceKeysConfigV0 {
                keys: level2_specs,
                emergency: level2_access(&l2.emergency),
                protocol: level2_access(&l2.protocol),
                election_difficulty: level2_access(&l2.election_difficulty),
                euro_per_energy: level2_access(&l2.euro_per_energy),
                micro_ccd_per_euro: level2_access(&l2.micro_ccd_per_euro),
                foundation_account: level2_access(&l2.foundation_account),
                mint_distribution: level2_access(&l2.mint_distribution),
                transaction_fee_distribution: level2_access(&l2.transaction_fee_distribution),
                gas_rewards: level2_access(&l2.gas_rewards),
                pool_parameters: level2_access(&l2.pool_parameters),
                add_anonymity_revoker: level2_access(&l2.add_anonymity_revoker),
                add_identity_provider: level2_access(&l2.add_identity_provider),
            },
        },
    )))
}

fn convert_key_specs(specs: &[HigherLevelKey]) -> anyhow::Result<Vec<GovernanceKeySpec>> {
    specs
        .iter()
        .map(|k| match k {
            HigherLevelKey::Existing { source } => {
                let data = std::fs::read(source).context(format!(
                    "Could not read governance key file: {}",
                    source.display()
                ))?;
                let key: UpdatePublicKey =
                    serde_json::from_slice(&data).context("Could not parse update key.")?;
                Ok(GovernanceKeySpec::Existing(key))
            }
            HigherLevelKey::Fresh { repeat } => Ok(GovernanceKeySpec::Fresh { count: *repeat }),
        })
        .collect()
}

fn level2_access(cfg: &Level2UpdateConfig) -> Level2AccessConfig {
    Level2AccessConfig {
        authorized_keys: cfg.authorized_keys.clone(),
        threshold: cfg.threshold,
    }
}

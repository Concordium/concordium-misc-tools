// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use anyhow::Context;
use concordium_rust_sdk::{
    base::{
        random_oracle::RandomOracle,
        sigma_protocols::{common::prove, dlog},
    },
    common::{
        types::{KeyIndex, KeyPair, TransactionTime},
        Versioned, VERSION_0,
    },
    id::{
        account_holder::{create_credential, generate_pio_v1},
        constants::{ArCurve, AttributeKind, IpPairing},
        curve_arithmetic::Curve,
        dodis_yampolskiy_prf as prf,
        pedersen_commitment::Value as PedersenValue,
        secret_sharing::Threshold,
        types::{
            account_address_from_registration_id, AccCredentialInfo, AccountCredential,
            AccountCredentialMessage, AccountKeys, ArInfos, CredentialData, CredentialHolderInfo,
            GlobalContext, HasIdentityObjectFields, IdCredentials, IdObjectUseData,
            IdRecoveryRequest, IdentityObjectV1, IpContext, IpInfo, Policy,
        },
    },
    smart_contracts::common::{AccountAddress, SignatureThreshold},
    types::{
        transactions::{BlockItem, Payload},
        AggregateSigPairing, BlockItemSummaryDetails, CredentialRegistrationID,
    },
    v2,
};
use either::Either;
use key_derivation::{words_to_seed, ConcordiumHdWallet, CredentialContext, Net};
use rand::*;
use serde::{ser::SerializeStruct, Serialize};
use serde_json::json;
use std::{collections::BTreeMap, str::FromStr};
use tauri::{api::dialog::blocking::FileDialogBuilder, async_runtime::Mutex};
use tonic::transport::ClientTlsConfig;

// All the resources are included in the binary as strings. This is done to
// avoid having to ship the resources separately. The resources are all taken
// from the concordium-infra-genesis-data repository.
const TESTNET_ANONYMITY_REVOKERS: &str = include_str!("../resources/testnet/ars.json");
const TESTNET_CRYPTOGRAPHIC_PARAMETERS: &str =
    include_str!("../resources/testnet/cryptographic-parameters.json");
const TESTNET_IP_INFO: &str = include_str!("../resources/testnet/ip-info.json");
const MAINNET_ANONYMITY_REVOKERS: &str = include_str!("../resources/mainnet/ars.json");
const MAINNET_CRYPTOGRAPHIC_PARAMETERS: &str =
    include_str!("../resources/mainnet/cryptographic-parameters.json");
const MAINNET_IP_INFO: &str = include_str!("../resources/mainnet/ip-info.json");

// https://github.com/Concordium/concordium-infra-genesis-data/blob/master/testnet/2022-06-13/genesis_data/genesis_hash
const TESTNET_GENESIS_HASH: &str =
    "4221332d34e1694168c2a0c0b3fd0f273809612cb13d000d5c2e00e85f50f796";
// https://github.com/Concordium/concordium-infra-genesis-data/blob/master/mainnet/2021-06-09/genesis_hash
const MAINNET_GENESIS_HASH: &str =
    "9dd9ca4d19e9393877d2c44b70f89acbfc0883c2243e5eeaecc0d1cd0503f478";

struct NetData {
    ip_info:    IpInfo<AggregateSigPairing>,
    ars:        ArInfos<ArCurve>,
    global_ctx: GlobalContext<ArCurve>,
}

struct State {
    seedphrase:   String,
    testnet_data: NetData,
    mainnet_data: NetData,
    node:         Mutex<Option<v2::Client>>,
    net:          Mutex<Option<Net>>,
    id_index:     Mutex<Option<u32>>,
}

/// A wallet and additional context.
struct WalletContext<'state> {
    wallet:     ConcordiumHdWallet,
    ip_context: IpContext<'state, AggregateSigPairing, ArCurve>,
    id_index:   Option<u32>,
    ip_index:   u32,
}

impl State {
    async fn wallet_context<'state>(
        &'state self,
        seedphrase: &str,
    ) -> Result<WalletContext<'state>, Error> {
        let bip39_map = client_server_helpers::bip39_map();
        let seed_words: Vec<_> = seedphrase.split_whitespace().map(String::from).collect();
        let seedphrase = seed_words.join(" ");
        if !client_server_helpers::verify_bip39(&seed_words, &bip39_map) {
            return Err(Error::InvalidSeedphrase);
        }
        let wallet = ConcordiumHdWallet {
            seed: words_to_seed(&seedphrase),
            net:  self.net.lock().await.context("No network set.")?,
        };

        let net_data = match wallet.net {
            Net::Mainnet => &self.mainnet_data,
            Net::Testnet => &self.testnet_data,
        };

        let id_index = *self.id_index.lock().await;
        let ip_index = net_data.ip_info.ip_identity.0;

        let context = IpContext::new(
            &net_data.ip_info,
            &net_data.ars.anonymity_revokers,
            &net_data.global_ctx,
        );

        Ok(WalletContext {
            wallet,
            ip_context: context,
            id_index,
            ip_index,
        })
    }
}

#[derive(Debug, thiserror::Error, strum::IntoStaticStr)]
enum Error {
    #[error("Error connecting to node: {0}")]
    Connection(#[from] tonic::transport::Error),
    #[error("Error querying node: {0}")]
    Query(#[from] v2::QueryError),
    #[error("Failed to create file: {0}")]
    FileError(#[from] Box<dyn std::error::Error>),
    #[error("Node is not on the {0} network.")]
    WrongNetwork(Net),
    #[error("Node is not caught up. Please try again later.")]
    NotCaughtUp,
    #[error("Invalid identity object: {0}")]
    InvalidIdObject(#[from] serde_json::Error),
    #[error("Invalid seedphrase.")]
    InvalidSeedphrase,
    #[error("The seedphrase does not match the identity object.")]
    SeedphraseIdObjectMismatch,
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
    #[error("You have used all possible accounts for this identity.")]
    TooManyAccounts,
}

// Needs Serialize to be able to return it from a command
impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer, {
        let mut error = serializer.serialize_struct("Error", 2)?;
        error.serialize_field("type", <&str>::from(self))?;
        error.serialize_field("message", &self.to_string())?;
        error.end()
    }
}

#[tauri::command]
async fn set_node_and_network(
    state: tauri::State<'_, State>,
    endpoint: String,
    net: Net,
) -> Result<(), Error> {
    let endpoint = v2::Endpoint::from_str(&endpoint)?;
    let endpoint = if endpoint
        .uri()
        .scheme()
        .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
    {
        endpoint.tls_config(ClientTlsConfig::new())?
    } else {
        endpoint
    };
    let ep = endpoint.connect_timeout(std::time::Duration::from_secs(5));
    let mut client = v2::Client::new(ep).await?;

    // Check that the node is on the right network
    let genesis_hash = client.get_consensus_info().await?.genesis_block;
    let expected_genesis_hash = match net {
        Net::Mainnet => MAINNET_GENESIS_HASH,
        Net::Testnet => TESTNET_GENESIS_HASH,
    };
    if genesis_hash.to_string() != expected_genesis_hash {
        return Err(Error::WrongNetwork(net));
    }

    let time_since_last_finalized = client
        .get_block_info(v2::BlockIdentifier::LastFinal)
        .await?
        .response
        .block_slot_time
        .signed_duration_since(chrono::Utc::now());
    if time_since_last_finalized.abs().num_seconds() > 60 {
        return Err(Error::NotCaughtUp);
    }

    *state.node.lock().await = Some(client);
    *state.net.lock().await = Some(net);

    Ok(())
}

#[tauri::command]
fn get_seedphrase(state: tauri::State<'_, State>) -> String { state.seedphrase.clone() }

#[tauri::command]
fn check_seedphrase(state: tauri::State<'_, State>, seedphrase: String) -> bool {
    let seed_words: Vec<_> = seedphrase.split_whitespace().collect();
    seed_words.join(" ") == state.seedphrase
}

/// Generate a request file for identity object. Needs to be async because the
/// file dialog blocks the thread.
#[tauri::command]
async fn save_request_file(state: tauri::State<'_, State>, net: Net) -> Result<(), Error> {
    *state.net.lock().await = Some(net);
    let WalletContext {
        wallet,
        ip_context,
        ip_index,
        ..
    } = state.wallet_context(&state.seedphrase).await?;
    let id_index = 0;

    let prf_key: prf::SecretKey<ArCurve> = wallet
        .get_prf_key(ip_index, id_index)
        .context("Could not get prf key")?;

    let id_cred_scalar = wallet
        .get_id_cred_sec(ip_index, id_index)
        .context("Could not get idCredSec")?;

    let randomness = wallet
        .get_blinding_randomness(ip_index, id_index)
        .context("Could not get blinding randomness")?;

    let id_cred_sec: PedersenValue<ArCurve> = PedersenValue::new(id_cred_scalar);
    let id_cred: IdCredentials<ArCurve> = IdCredentials { id_cred_sec };
    let cred_holder_info = CredentialHolderInfo::<ArCurve> { id_cred };

    let aci = AccCredentialInfo {
        cred_holder_info,
        prf_key,
    };
    let id_use_data = IdObjectUseData { aci, randomness };

    let threshold = Threshold(ip_context.ars_infos.len() as u8 - 1);
    let (pio, _) = generate_pio_v1(&ip_context, threshold, &id_use_data)
        .context("Failed to generate the identity object request.")?;
    let ver_pio = Versioned::new(VERSION_0, pio);

    let mut file_builder = FileDialogBuilder::new()
        .set_file_name("request.json")
        .set_title("Save request file")
        .add_filter("JSON", &["json"]);
    if let Ok(cd) = std::env::current_dir() {
        file_builder = file_builder.set_directory(cd);
    } else {
        let user_dirs = directories::UserDirs::new();
        if let Some(documents) = user_dirs
            .as_ref()
            .and_then(directories::UserDirs::document_dir)
        {
            file_builder = file_builder.set_directory(documents);
        }
    };
    let Some(path) = file_builder
        .save_file()
    else {
        return Ok(());
    };

    let file = std::fs::File::create(path).context("Could not create file.")?;
    serde_json::to_writer_pretty(file, &ver_pio).context("Could not write to file.")?;
    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    id_index:  u32,
    acc_index: u8,
    address:   AccountAddress,
}

#[derive(serde::Serialize)]
struct IdentityData {
    accounts:   Vec<Account>,
    attributes: Vec<(String, String)>,
}

/// When finding all accounts for an identity, we stop after this many failures,
/// i.e. if we have not found any accounts for this many consecutive indices.
const MAX_ACCOUNT_FAILURES: u8 = 20;

#[tauri::command]
async fn get_identity_data(
    state: tauri::State<'_, State>,
    seedphrase: String,
    id_object: String,
) -> Result<IdentityData, Error> {
    let id_object = parse_id_object(id_object)?;
    let WalletContext {
        wallet, ip_context, ..
    } = state.wallet_context(&seedphrase).await?;

    // Find the identity index associated with the identity object. We do this
    // by computing the public key for each index and comparing it to the
    // identity object's public key. If we do not find a match after 20
    // iterations, we give up and return an error.
    let id_index = (0..20)
        .find_map(|id_idx| {
            let id_cred_sec = wallet
                .get_id_cred_sec(ip_context.ip_info.ip_identity.0, id_idx)
                .ok()?;
            let g = ip_context.global_context.on_chain_commitment_key.g;
            let id_cred_pub = g.mul_by_scalar(&id_cred_sec);
            if id_cred_pub == id_object.pre_identity_object.id_cred_pub {
                Some(id_idx)
            } else {
                None
            }
        })
        .ok_or(Error::SeedphraseIdObjectMismatch)?;
    *state.id_index.lock().await = Some(id_index);

    let mut accounts = Vec::new();
    let mut client = state.node.lock().await;
    let client = client.as_mut().context("No node set.")?;
    let ip_index = ip_context.ip_info.ip_identity.0;
    let mut acc_fail_count = 0;
    for acc_index in 0u8..=id_object.alist.max_accounts {
        let address = {
            // This needs to be in a separate scope to avoid keeping prf_key across an await
            // boundary
            let prf_key = wallet
                .get_prf_key(ip_index, id_index)
                .context("Failed to get PRF key.")?;
            let reg_id = prf_key
                .prf(ip_context.global_context.elgamal_generator(), acc_index)
                .context("Failed to compute PRF.")?;
            account_address_from_registration_id(&reg_id)
        };
        match client
            .get_account_info(&address.into(), v2::BlockIdentifier::LastFinal)
            .await
        {
            Ok(_) => {
                let account = Account {
                    id_index,
                    acc_index,
                    address,
                };
                accounts.push(account);
                acc_fail_count = 0;
            }
            Err(e) if e.is_not_found() => {
                acc_fail_count += 1;
                if acc_fail_count > MAX_ACCOUNT_FAILURES {
                    break;
                }
            }
            Err(e) => return Err(anyhow::anyhow!("Cannot query the node: {e}").into()),
        }
    }

    let attributes = id_object
        .alist
        .alist
        .into_iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    Ok(IdentityData {
        accounts,
        attributes,
    })
}

#[tauri::command]
async fn create_account(
    state: tauri::State<'_, State>,
    seedphrase: String,
    id_object: String,
    acc_index: u8,
) -> Result<Account, Error> {
    let id_object = parse_id_object(id_object)?;
    let wallet_context = state.wallet_context(&seedphrase).await?;
    let id_index = wallet_context.id_index.context("Identity index not set.")?;
    let net = wallet_context.wallet.net;

    if acc_index > id_object.alist.max_accounts {
        return Err(Error::TooManyAccounts);
    }

    let (acc_cred_msg, acc_keys) =
        get_credential_deployment_info(id_object, wallet_context, acc_index).await?;

    // Submit the credential to the chain
    let bi = BlockItem::<Payload>::CredentialDeployment(Box::new(acc_cred_msg));
    let mut client = state.node.lock().await;
    let client = client.as_mut().context("No node set.")?;
    let tx = client
        .send_block_item(&bi)
        .await
        .context("Failed to deploy credential.")?;

    let (_, summary) = client
        .wait_until_finalized(&tx)
        .await
        .context("Failed to wait for finalization.")?;
    let BlockItemSummaryDetails::AccountCreation(details) = summary.details else {
        return Err(anyhow::anyhow!("Block item was not an account creation.").into());
    };

    let file_data = json!({
        "type": "concordium-browser-wallet-account",
        "v": 0,
        "environment": match net {
            Net::Mainnet => "mainnet",
            Net::Testnet => "testnet",
        },
        "value": {
            "accountKeys": acc_keys,
            "credentials": {
                "0": details.reg_id
            },
            "address": details.address
        }
    });

    let account = Account {
        id_index,
        acc_index,
        address: details.address,
    };

    let Some(path) = FileDialogBuilder::new()
        .set_file_name("account-keys.json")
        .set_title("Save account key file")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return Ok(account);
    };

    let file = std::fs::File::create(path).map_err(|e| Error::FileError(e.into()))?;
    serde_json::to_writer_pretty(file, &file_data).map_err(|e| Error::FileError(e.into()))?;

    Ok(account)
}

async fn get_credential_deployment_info(
    id_obj: IdentityObjectV1<IpPairing, ArCurve, AttributeKind>,
    wallet_context: WalletContext<'_>,
    acc_index: u8,
) -> Result<
    (
        AccountCredentialMessage<IpPairing, ArCurve, AttributeKind>,
        AccountKeys,
    ),
    Error,
> {
    let WalletContext {
        wallet,
        ip_context,
        id_index,
        ip_index,
        ..
    } = wallet_context;
    let id_index = id_index.context("Identity index not set.")?;

    let policy: Policy<ArCurve, AttributeKind> = Policy {
        valid_to:   id_obj.get_attribute_list().valid_to,
        created_at: id_obj.get_attribute_list().created_at,
        policy_vec: BTreeMap::new(),
        _phantom:   Default::default(),
    };

    let randomness = wallet
        .get_blinding_randomness(ip_index, id_index)
        .context("Failed to get blinding randomness.")?;
    let prf_key = wallet
        .get_prf_key(ip_index, id_index)
        .context("Failed to get PRF key.")?;
    let id_cred_sec_scalar = wallet
        .get_id_cred_sec(ip_index, id_index)
        .context("Failed to get idCredSec.")?;

    let id_cred_sec = PedersenValue::new(id_cred_sec_scalar);
    let id_cred = IdCredentials { id_cred_sec };
    let cred_holder_info = CredentialHolderInfo::<ArCurve> { id_cred };
    let aci = AccCredentialInfo {
        cred_holder_info,
        prf_key,
    };
    let id_use_data = IdObjectUseData { aci, randomness };

    let acc_data = get_account_keys(&wallet, ip_index, id_index, acc_index)?;
    let credential_context = CredentialContext {
        wallet,
        identity_provider_index: ip_index.into(),
        identity_index: id_index,
        credential_index: acc_index,
    };

    let message_expiry = TransactionTime::minutes_after(5);
    let (cdi, _) = create_credential(
        ip_context,
        &id_obj,
        &id_use_data,
        acc_index,
        policy,
        &acc_data,
        &credential_context,
        &Either::Left(message_expiry),
    )
    .context("Could not generate the credential.")?;

    let acc_keys: AccountKeys = acc_data.into();
    let acc_cred_msg = AccountCredentialMessage {
        message_expiry,
        credential: AccountCredential::Normal { cdi },
    };
    Ok((acc_cred_msg, acc_keys))
}

fn get_account_keys(
    wallet: &ConcordiumHdWallet,
    ip_index: u32,
    id_index: u32,
    acc_index: u8,
) -> Result<CredentialData, Error> {
    let secret = wallet
        .get_account_signing_key(ip_index, id_index, acc_index as u32)
        .context("Failed to get account signing key.")?;
    let public = (&secret).into();
    let mut keys = std::collections::BTreeMap::new();
    keys.insert(KeyIndex(0), KeyPair { secret, public });

    Ok(CredentialData {
        keys,
        threshold: SignatureThreshold::ONE,
    })
}

#[tauri::command]
async fn save_keys(
    state: tauri::State<'_, State>,
    seedphrase: String,
    account: Account,
) -> Result<(), Error> {
    let WalletContext {
        wallet,
        ip_context,
        ip_index,
        ..
    } = state.wallet_context(&seedphrase).await?;
    let net = wallet.net;

    let acc_data = get_account_keys(&wallet, ip_index, account.id_index, account.acc_index)?;
    let acc_keys: AccountKeys = acc_data.into();

    let prf_key = wallet
        .get_prf_key(ip_index, account.id_index)
        .context("Failed to get PRF key.")?;
    let cred_id_exp = prf_key
        .prf_exponent(account.acc_index)
        .context("Failed to compute PRF exponent.")?;
    let cred_reg_id =
        CredentialRegistrationID::from_exponent(&ip_context.global_context, cred_id_exp);

    let file_data = json!({
        "type": "concordium-browser-wallet-account",
        "v": 0,
        "environment": match net {
            Net::Mainnet => "mainnet",
            Net::Testnet => "testnet",
        },
        "value": {
            "accountKeys": acc_keys,
            "credentials": {
                "0": cred_reg_id
            },
            "address": account.address
        }
    });

    let Some(path) = FileDialogBuilder::new()
        .set_file_name("account-keys.json")
        .set_title("Save account key file")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return Ok(());
    };

    let file = std::fs::File::create(path).map_err(|e| Error::FileError(e.into()))?;
    serde_json::to_writer_pretty(file, &file_data).map_err(|e| Error::FileError(e.into()))?;

    Ok(())
}

/// When finding all identities for a seedphrase, we stop after this many
/// failures, i.e. if we have failed to find accounts for a candidate identity
/// this many times.
const MAX_IDENTITY_FAILURES: u8 = 20;

#[tauri::command]
async fn recover_identities(
    state: tauri::State<'_, State>,
    seedphrase: String,
) -> Result<Vec<Account>, Error> {
    let WalletContext {
        wallet,
        ip_context,
        ip_index,
        ..
    } = state.wallet_context(&seedphrase).await?;

    let mut accounts = Vec::new();
    let mut client = state.node.lock().await;
    let client = client.as_mut().context("No node set.")?;

    let mut id_fail_count = 0;
    'id_loop: for id_index in 0.. {
        let mut acc_fail_count = 0;
        for acc_index in 0u8..=255 {
            let address = {
                // This needs to be in a separate scope to avoid keeping prf_key across an await
                // boundary
                let prf_key = wallet
                    .get_prf_key(ip_index, id_index)
                    .context("Failed to get PRF key.")?;
                let reg_id = prf_key
                    .prf(ip_context.global_context.elgamal_generator(), acc_index)
                    .context("Failed to compute PRF.")?;
                account_address_from_registration_id(&reg_id)
            };
            match client
                .get_account_info(&address.into(), v2::BlockIdentifier::LastFinal)
                .await
            {
                Ok(_) => {
                    let account = Account {
                        id_index,
                        acc_index,
                        address,
                    };
                    accounts.push(account);
                    acc_fail_count = 0;
                    id_fail_count = 0;
                }
                Err(e) if e.is_not_found() => {
                    acc_fail_count += 1;
                    if acc_fail_count > MAX_ACCOUNT_FAILURES {
                        id_fail_count += 1;
                        if id_fail_count > MAX_IDENTITY_FAILURES {
                            break 'id_loop;
                        }
                        break;
                    }
                }
                Err(e) => return Err(anyhow::anyhow!("Cannot query the node: {e}").into()),
            }
        }
    }

    Ok(accounts)
}

#[tauri::command]
async fn generate_recovery_request(
    state: tauri::State<'_, State>,
    seedphrase: String,
    id_index: u32,
) -> Result<(), Error> {
    let WalletContext {
        wallet,
        ip_context,
        ip_index,
        ..
    } = state.wallet_context(&seedphrase).await?;

    let id_cred_scalar = wallet
        .get_id_cred_sec(ip_index, id_index)
        .context("Could not get idCredSec")?;
    let id_cred_sec = PedersenValue::new(id_cred_scalar);

    let g = ip_context.global_context.on_chain_commitment_key.g;
    let id_cred_pub = g.mul_by_scalar(&id_cred_sec);
    let prover = dlog::Dlog {
        public: id_cred_pub,
        coeff:  g,
    };
    let secret = dlog::DlogSecret {
        secret: id_cred_sec,
    };
    let timestamp = chrono::Utc::now().timestamp() as u64;

    let mut csprng = thread_rng();
    let mut transcript = RandomOracle::domain("IdRecoveryProof");
    transcript.append_message(b"ctx", &ip_context.global_context);
    transcript.append_message(b"timestamp", &timestamp);
    transcript.append_message(b"ipIdentity", &ip_context.ip_info.ip_identity);
    transcript.append_message(b"ipVerifyKey", &ip_context.ip_info.ip_verify_key);
    let proof = prove(&mut transcript, &prover, secret, &mut csprng)
        .context("Failed to generate recovery proof.")?;

    let request = IdRecoveryRequest {
        id_cred_pub,
        timestamp,
        proof,
    };
    let json = Versioned {
        version: VERSION_0,
        value:   request,
    };

    let Some(path) = FileDialogBuilder::new()
        .set_file_name("recovery-request.json")
        .set_title("Save recovery request file")
        .add_filter("JSON", &["json"])
        .save_file()
    else {
        return Ok(());
    };

    let file = std::fs::File::create(path).map_err(|e| Error::FileError(e.into()))?;
    serde_json::to_writer_pretty(file, &json).map_err(|e| Error::FileError(e.into()))?;

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let bip39_vec = client_server_helpers::bip39_words().collect::<Vec<_>>();
    let words = client_server_helpers::rerandomize_bip39(&[], &bip39_vec).unwrap();
    let seedphrase = words.join(" ");

    let state = State {
        seedphrase,
        testnet_data: parse_network_data(
            TESTNET_IP_INFO,
            TESTNET_ANONYMITY_REVOKERS,
            TESTNET_CRYPTOGRAPHIC_PARAMETERS,
        )?,
        mainnet_data: parse_network_data(
            MAINNET_IP_INFO,
            MAINNET_ANONYMITY_REVOKERS,
            MAINNET_CRYPTOGRAPHIC_PARAMETERS,
        )?,
        node: Mutex::new(None),
        net: Mutex::new(None),
        id_index: Mutex::new(None),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_seedphrase,
            check_seedphrase,
            save_request_file,
            set_node_and_network,
            get_identity_data,
            create_account,
            save_keys,
            recover_identities,
            generate_recovery_request,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    Ok(())
}

fn parse_id_object(
    id_object: String,
) -> Result<IdentityObjectV1<AggregateSigPairing, ArCurve, AttributeKind>, Error> {
    let id_obj: Versioned<serde_json::Value> = serde_json::from_str(&id_object)?;
    if id_obj.version != VERSION_0 {
        return Err(
            anyhow::anyhow!("Unsupported version of identity object: {}", id_obj.version).into(),
        );
    }
    serde_json::from_value(id_obj.value).map_err(Error::from)
}

fn parse_network_data(
    ip_info: &str,
    anonymity_revokers: &str,
    cryptographic_parameters: &str,
) -> anyhow::Result<NetData> {
    let ip_info: Versioned<serde_json::Value> =
        serde_json::from_str(ip_info).context("Error parsing ip info")?;
    anyhow::ensure!(
        ip_info.version.value == 0,
        "Unsupported version of ip info: {}",
        ip_info.version
    );
    let ip_info = serde_json::from_value(ip_info.value).context("Error parsing ip info")?;

    let ars: Versioned<serde_json::Value> =
        serde_json::from_str(anonymity_revokers).context("Error parsing anonymity revokers")?;
    anyhow::ensure!(
        ars.version.value == 0,
        "Unsupported version of anonymity revokers: {}",
        ars.version
    );
    let ars = serde_json::from_value(ars.value).context("Error parsing anonymity revokers")?;

    let global_ctx: Versioned<serde_json::Value> =
        serde_json::from_str(cryptographic_parameters)
            .context("Error parsing cryptographic parameters")?;
    anyhow::ensure!(
        global_ctx.version.value == 0,
        "Unsupported version of cryptographic parameters: {}",
        global_ctx.version
    );
    let global_ctx = serde_json::from_value(global_ctx.value)
        .context("Error parsing cryptographic parameters")?;

    Ok(NetData {
        ip_info,
        ars,
        global_ctx,
    })
}

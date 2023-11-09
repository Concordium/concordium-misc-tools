// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{collections::BTreeMap, str::FromStr};

use anyhow::Context;
use concordium_rust_sdk::{
    common::{
        types::{CredentialIndex, KeyIndex, KeyPair, TransactionTime},
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
            IdentityObjectV1, IpContext, IpInfo, Policy,
        },
    },
    smart_contracts::common::{AccountAddress, SignatureThreshold},
    types::{
        transactions::{BlockItem, Payload},
        AccountThreshold, BlockItemSummaryDetails,
    },
    v2,
};
use either::Either;
use key_derivation::{words_to_seed, ConcordiumHdWallet, CredentialContext, Net};
use pairing::bls12_381::Bls12;
use serde::Serialize;
use serde_json::json;
use tauri::{api::dialog::blocking::FileDialogBuilder, async_runtime::Mutex};
use tonic::transport::ClientTlsConfig;

mod bip39;

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

#[derive(serde::Serialize)]
struct NetData {
    ip_info:    IpInfo<Bls12>,
    ars:        ArInfos<ArCurve>,
    global_ctx: GlobalContext<ArCurve>,
}

struct State {
    seedphrase:   String,
    testnet_data: NetData,
    mainnet_data: NetData,
    node:         Mutex<Option<v2::Client>>,
    net:          Mutex<Option<Net>>,
    account_keys: Mutex<Option<serde_json::Value>>,
    id_index:     Mutex<Option<u32>>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Error connecting to node: {0}")]
    Connection(#[from] tonic::transport::Error),
    #[error("Error querying node: {0}")]
    Query(#[from] v2::QueryError),
    #[error("Failed to create file: {0}")]
    FileError(#[from] Box<dyn std::error::Error>),
    #[error("Node is not on the {0} network.")]
    WrongNetwork(Net),
    #[error("Invalid identity object: {0}")]
    InvalidIdObject(#[from] serde_json::Error),
    #[error("Invalid seedphrase.")]
    InvalidSeedphrase,
    #[error("The seedphrase does not match the identity object.")]
    SeedphraseIdObjectMismatch,
    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

// Needs Serialize to be able to return it from a command
impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer, {
        self.to_string().serialize(serializer)
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
    let mut client = v2::Client::new(endpoint).await?;

    // Check that the node is on the right network
    let genesis_hash = client.get_consensus_info().await?.genesis_block;
    let expected_genesis_hash = match net {
        Net::Mainnet => MAINNET_GENESIS_HASH,
        Net::Testnet => TESTNET_GENESIS_HASH,
    };
    if genesis_hash.to_string() != expected_genesis_hash {
        return Err(Error::WrongNetwork(net));
    }

    *state.node.lock().await = Some(client);
    *state.net.lock().await = Some(net);

    Ok(())
}

#[tauri::command]
fn get_seedphrase(state: tauri::State<'_, State>) -> String { state.seedphrase.clone() }

#[tauri::command]
fn check_seedphrase(state: tauri::State<'_, State>, seedphrase: String) -> bool {
    seedphrase.trim() == state.seedphrase
}

/// Generate a request file for identity object. Needs to be async because the
/// file dialog blocks the thread.
#[tauri::command]
async fn save_request_file(state: tauri::State<'_, State>, net: Net) -> Result<(), String> {
    let net_data = match net {
        Net::Mainnet => &state.mainnet_data,
        Net::Testnet => &state.testnet_data,
    };

    let context = IpContext::new(
        &net_data.ip_info,
        &net_data.ars.anonymity_revokers,
        &net_data.global_ctx,
    );

    let wallet = ConcordiumHdWallet {
        seed: words_to_seed(&state.seedphrase),
        net,
    };
    let identity_provider_index = net_data.ip_info.ip_identity.0;
    let identity_index = 0;

    let prf_key: prf::SecretKey<ArCurve> = wallet
        .get_prf_key(identity_provider_index, identity_index)
        .map_err(|e| format!("Could not get prf key: {e}"))?;

    let id_cred_scalar = wallet
        .get_id_cred_sec(identity_provider_index, identity_index)
        .map_err(|e| format!("Could not get idCredSec: {e}"))?;

    let randomness = wallet
        .get_blinding_randomness(identity_provider_index, identity_index)
        .map_err(|e| format!("Could not get blinding randomness: {e}"))?;

    let id_cred_sec: PedersenValue<ArCurve> = PedersenValue::new(id_cred_scalar);
    let id_cred: IdCredentials<ArCurve> = IdCredentials { id_cred_sec };
    let cred_holder_info = CredentialHolderInfo::<ArCurve> { id_cred };

    let aci = AccCredentialInfo {
        cred_holder_info,
        prf_key,
    };
    let id_use_data = IdObjectUseData { aci, randomness };

    let threshold = Threshold(net_data.ars.anonymity_revokers.len() as u8 - 1);
    let (pio, _) = generate_pio_v1(&context, threshold, &id_use_data)
        .ok_or_else(|| "Failed to generate the identity object request.".to_string())?;
    let ver_pio = Versioned::new(VERSION_0, pio);

    let Some(path) = FileDialogBuilder::new()
        .set_file_name("request.json")
        .set_title("Save request file")
        .save_file()
    else {
        return Ok(());
    };

    let file = std::fs::File::create(path).map_err(|e| format!("Could not create file: {e}"))?;
    serde_json::to_writer_pretty(file, &ver_pio)
        .map_err(|e| format!("Could not write to file: {e}"))
}

const MAX_ACCOUNT_FAILURES: u8 = 20;

#[derive(serde::Serialize)]
struct Account {
    index:   u8,
    address: AccountAddress,
}

#[tauri::command]
async fn get_identity_accounts(
    state: tauri::State<'_, State>,
    seedphrase: String,
    id_object: String,
) -> Result<Vec<Account>, Error> {
    let id_object = parse_id_object(id_object)?;
    let bip39_map = bip39::bip39_map();
    let seedphrase = seedphrase.trim();
    let seed_words: Vec<_> = seedphrase.split(' ').collect();
    if !bip39::verify_bip39(&seed_words, &bip39_map) {
        return Err(Error::InvalidSeedphrase);
    }
    let wallet = ConcordiumHdWallet {
        seed: words_to_seed(&seedphrase),
        net:  state.net.lock().await.context("No network set.")?,
    };

    let net_data = match wallet.net {
        Net::Mainnet => &state.mainnet_data,
        Net::Testnet => &state.testnet_data,
    };

    // Find the identity index asssoicated with the identity object
    let id_index = (0..20)
        .find_map(|id_idx| {
            let id_cred_sec = wallet
                .get_id_cred_sec(net_data.ip_info.ip_identity.0, id_idx)
                .ok()?;
            let g = net_data.global_ctx.on_chain_commitment_key.g;
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
    let ip_index = net_data.ip_info.ip_identity.0;
    let mut acc_fail_count = 0;
    for acc_idx in 0u8..=255 {
        // This needs to be in a separate scope to avoid keeping prf_key across an await
        // boundary
        let address = {
            let prf_key = wallet
                .get_prf_key(ip_index, id_index)
                .context("Failed to get PRF key.")?;
            let reg_id = prf_key
                .prf(net_data.global_ctx.elgamal_generator(), acc_idx)
                .context("Failed to compute PRF.")?;
            account_address_from_registration_id(&reg_id)
        };
        match client
            .get_account_info(&address.into(), v2::BlockIdentifier::LastFinal)
            .await
        {
            Ok(_) => {
                let account = Account {
                    index: acc_idx,
                    address,
                };
                accounts.push(account);
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

    Ok(accounts)
}

#[tauri::command]
async fn create_account(
    state: tauri::State<'_, State>,
    seedphrase: String,
    id_object: String,
    acc_index: u8,
) -> Result<Account, Error> {
    let id_object = parse_id_object(id_object)?;
    let bip39_map = bip39::bip39_map();
    let seedphrase = seedphrase.trim();
    let seed_words: Vec<_> = seedphrase.split(' ').collect();
    if !bip39::verify_bip39(&seed_words, &bip39_map) {
        return Err(Error::InvalidSeedphrase);
    }

    let net = state.net.lock().await.context("No network set.")?;
    let wallet = ConcordiumHdWallet {
        seed: words_to_seed(&seedphrase),
        net,
    };

    let (acc_cred_msg, acc_keys) =
        get_credential_deployment_info(state.inner(), id_object, wallet, acc_index).await?;

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
    *state.account_keys.lock().await = Some(file_data.clone());

    let account = Account {
        index:   acc_index,
        address: details.address,
    };

    let Some(path) = FileDialogBuilder::new()
        .set_file_name("account-keys.json")
        .set_title("Save account key file")
        .save_file()
    else {
        return Ok(account);
    };

    let file = std::fs::File::create(path).map_err(|e| Error::FileError(e.into()))?;
    serde_json::to_writer_pretty(file, &file_data).map_err(|e| Error::FileError(e.into()))?;

    Ok(account)
}

async fn get_credential_deployment_info(
    state: &State,
    id_obj: IdentityObjectV1<IpPairing, ArCurve, AttributeKind>,
    wallet: ConcordiumHdWallet,
    acc_index: u8,
) -> Result<
    (
        AccountCredentialMessage<IpPairing, ArCurve, AttributeKind>,
        AccountKeys,
    ),
    Error,
> {
    let net_data = match wallet.net {
        Net::Mainnet => &state.mainnet_data,
        Net::Testnet => &state.testnet_data,
    };

    let ip_index = net_data.ip_info.ip_identity.0;
    let id_index = state
        .id_index
        .lock()
        .await
        .context("Identity index was not initialized")?;

    let policy: Policy<ArCurve, AttributeKind> = Policy {
        valid_to:   id_obj.get_attribute_list().valid_to,
        created_at: id_obj.get_attribute_list().created_at,
        policy_vec: BTreeMap::new(),
        _phantom:   Default::default(),
    };

    let context = IpContext::new(
        &net_data.ip_info,
        &net_data.ars.anonymity_revokers,
        &net_data.global_ctx,
    );

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

    let secret = wallet
        .get_account_signing_key(ip_index, id_index, acc_index as u32)
        .context("Failed to get account signing key.")?;
    let mut keys = std::collections::BTreeMap::new();
    let public = ed25519_dalek::PublicKey::from(&secret);
    keys.insert(KeyIndex(0), KeyPair { secret, public });
    let acc_data = CredentialData {
        keys,
        threshold: SignatureThreshold::ONE,
    };
    let credential_context = CredentialContext {
        wallet,
        identity_provider_index: ip_index.into(),
        identity_index: id_index,
        credential_index: acc_index,
    };

    let message_expiry = TransactionTime::seconds_after(5 * 60);
    let (cdi, _) = create_credential(
        context,
        &id_obj,
        &id_use_data,
        acc_index,
        policy,
        &acc_data,
        &credential_context,
        &Either::Left(message_expiry),
    )
    .context("Could not generate the credential.")?;

    let acc_keys = AccountKeys {
        keys:      [(CredentialIndex::from(0), acc_data)].into(),
        threshold: AccountThreshold::ONE,
    };

    let acc_cred_msg = AccountCredentialMessage {
        message_expiry,
        credential: AccountCredential::Normal { cdi },
    };
    Ok((acc_cred_msg, acc_keys))
}

fn main() -> anyhow::Result<()> {
    let bip39_vec = bip39::bip39_words().collect::<Vec<_>>();
    let words = bip39::rerandomize_bip39(&[], &bip39_vec).unwrap();
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
        account_keys: Mutex::new(None),
        id_index: Mutex::new(None),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_seedphrase,
            check_seedphrase,
            save_request_file,
            set_node_and_network,
            get_identity_accounts,
            create_account,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    Ok(())
}

fn parse_id_object(
    id_object: String,
) -> Result<IdentityObjectV1<Bls12, ArCurve, AttributeKind>, Error> {
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

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::str::FromStr;

use concordium_rust_sdk::{
    common::{Versioned, VERSION_0},
    id::{
        account_holder::generate_pio_v1,
        constants::ArCurve,
        dodis_yampolskiy_prf as prf,
        pedersen_commitment::Value as PedersenValue,
        secret_sharing::Threshold,
        types::{
            AccCredentialInfo, ArInfos, CredentialHolderInfo, GlobalContext, IdCredentials,
            IdObjectUseData, IpContext, IpInfo,
        },
    },
    v2,
};
use key_derivation::{words_to_seed, ConcordiumHdWallet, Net};
use pairing::bls12_381::Bls12;
use serde::Serialize;
use tauri::{api::dialog::blocking::FileDialogBuilder, async_runtime::Mutex};
use tonic::transport::ClientTlsConfig;

mod bip39;

const TESTNET_ANONYMITY_REVOKERS: &str = include_str!("../resources/testnet/ars.json");
const TESTNET_CRYPTOGRAPHIC_PARAMETERS: &str =
    include_str!("../resources/testnet/cryptographic-parameters.json");
const TESTNET_IP_INFO: &str = include_str!("../resources/testnet/ip-info.json");

// https://github.com/Concordium/concordium-infra-genesis-data/blob/master/testnet/2022-06-13/genesis_data/genesis_hash
const TESTNET_GENESIS_HASH: &str =
    "4221332d34e1694168c2a0c0b3fd0f273809612cb13d000d5c2e00e85f50f796";
// https://github.com/Concordium/concordium-infra-genesis-data/blob/master/mainnet/2021-06-09/genesis_hash
const MAINNET_GENESIS_HASH: &str =
    "9dd9ca4d19e9393877d2c44b70f89acbfc0883c2243e5eeaecc0d1cd0503f478";

struct State {
    seedphrase: String,
    node:       Mutex<Option<v2::Client>>,
    net:        Mutex<Option<Net>>,
}

#[derive(Debug, thiserror::Error)]
enum Error {
    #[error("Error connecting to node: {0}")]
    Connection(#[from] tonic::transport::Error),
    #[error("Error querying node: {0}")]
    Query(#[from] v2::QueryError),
    #[error("Node is not on the {0} network.")]
    WrongNetwork(Net),
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
    let ip_info: Versioned<IpInfo<Bls12>> =
        serde_json::from_str(TESTNET_IP_INFO).map_err(|e| format!("Error parsing ip info: {e}"))?;
    if ip_info.version.value != 0 {
        return Err(format!(
            "Unsupported version of ip info: {}",
            ip_info.version
        ));
    }
    let ip_info = ip_info.value;
    let ars: Versioned<ArInfos<ArCurve>> = serde_json::from_str(TESTNET_ANONYMITY_REVOKERS)
        .map_err(|e| format!("Error parsing anonymity revokers: {e}"))?;
    if ars.version.value != 0 {
        return Err(format!(
            "Unsupported version of anonymity revokers: {}",
            ars.version
        ));
    }
    let ars = ars.value;
    let global_ctx: Versioned<GlobalContext<ArCurve>> =
        serde_json::from_str(TESTNET_CRYPTOGRAPHIC_PARAMETERS)
            .map_err(|e| format!("Error parsing cryptographic parameters: {e}"))?;
    if global_ctx.version.value != 0 {
        return Err(format!(
            "Unsupported version of cryptographic parameters: {}",
            global_ctx.version
        ));
    }
    let global_ctx = global_ctx.value;
    let context = IpContext::new(&ip_info, &ars.anonymity_revokers, &global_ctx);

    let wallet = ConcordiumHdWallet {
        seed: words_to_seed(&state.seedphrase),
        net,
    };
    let identity_provider_index = ip_info.ip_identity.0;
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

    let threshold = Threshold(ars.anonymity_revokers.len() as u8 - 1);
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

fn main() -> Result<(), String> {
    let bip39_vec = bip39::bip39_words().collect::<Vec<_>>();
    let words = bip39::rerandomize_bip39(&[], &bip39_vec).unwrap();
    let seedphrase = words.join(" ");

    let state = State {
        seedphrase,
        node: Mutex::new(None),
        net: Mutex::new(None),
    };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_seedphrase,
            check_seedphrase,
            save_request_file,
            set_node_and_network,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    Ok(())
}

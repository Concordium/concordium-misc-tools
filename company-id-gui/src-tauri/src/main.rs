// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use concordium_base::{
    common::{Versioned, VERSION_0},
    dodis_yampolskiy_prf as prf,
    id::{
        account_holder::generate_pio_v1,
        constants::ArCurve,
        secret_sharing::Threshold,
        types::{
            AccCredentialInfo, ArInfos, CredentialHolderInfo, GlobalContext, IdCredentials,
            IdObjectUseData, IpContext, IpInfo,
        },
    },
    pedersen_commitment::Value as PedersenValue,
};
use key_derivation::{words_to_seed, ConcordiumHdWallet, Net};
use pairing::bls12_381::Bls12;
use tauri::api::dialog::blocking::FileDialogBuilder;

mod bip39;

const TESTNET_ANONYMITY_REVOKERS: &str = include_str!("../resources/testnet/ars.json");
const TESTNET_CRYPTOGRAPHIC_PARAMETERS: &str =
    include_str!("../resources/testnet/cryptographic-parameters.json");
const TESTNET_IP_INFO: &str = include_str!("../resources/testnet/ip-info.json");

struct State {
    seedphrase: String,
}

#[tauri::command]
fn get_seedphrase(state: tauri::State<State>) -> String { state.seedphrase.clone() }

#[tauri::command]
fn check_seedphrase(state: tauri::State<State>, seedphrase: String) -> bool {
    state.seedphrase == seedphrase
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
    let words = bip39::rerandomize_bip39(&[], &bip39_vec)?;
    let seedphrase = words.join(" ");

    let state = State { seedphrase };

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            get_seedphrase,
            check_seedphrase,
            save_request_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
    Ok(())
}

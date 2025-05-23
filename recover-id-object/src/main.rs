use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use concordium::{
    common::{base16_decode_string, base16_encode_string, Versioned, VERSION_0},
    id::{
        account_holder::generate_id_recovery_request,
        constants::{ArCurve, AttributeKind, IpPairing},
        pedersen_commitment::Value as PedersenValue,
        types::{GlobalContext, IdRecoveryRequest, IdentityObjectV1, IpInfo},
    },
    types::CredentialRegistrationID,
    v2::{self, AccountIdentifier, BlockIdentifier},
};
use concordium_rust_sdk as concordium;
use key_derivation::{ConcordiumHdWallet, PrfKey};
use tonic::transport::ClientTlsConfig;

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Api {
    #[clap(
        long = "concordium-api",
        name = "concordium-api",
        help = "GRPC V2 interface of the Concordium node.",
        default_value = "http://localhost:20000",
        global = true
    )]
    api: concordium::v2::Endpoint,
    /// Request timeout for Concordium node requests.
    #[clap(
        long,
        help = "Timeout for requests to the Concordium node.",
        default_value = "10",
        global = true
    )]
    concordium_request_timeout: u64,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    #[clap(about = "Generate secrets for a given seed phrase.")]
    GenerateSecrets(GenerateSecretsArgs),
    #[clap(about = "Recover an identity from a seed phrase or generated secrets.")]
    RecoverIdentity(RecoverIdentityArgs),
}

#[derive(Debug, Args)]
struct GenerateSecretsArgs {
    /// Location of the seed phrase.
    #[clap(long, help = "Path to the seed phrase file.")]
    concordium_wallet: std::path::PathBuf,
    #[clap(long = "ip-index", help = "Identity of the identity provider.")]
    ip_index:          u32,
    #[clap(
        long = "id-index",
        help = "Index of the identity to generate secrets for."
    )]
    id_index:          u32,
    #[clap(
        long,
        help = "Network to generate secrets for.",
        default_value = "testnet"
    )]
    network:           key_derivation::Net,
}

#[derive(Debug, Args)]
struct RecoverIdentityArgs {
    /// Recovery URL start.
    #[clap(
        long = "ip-info-url",
        help = "Identity recovery URL",
        default_value = "http://wallet-proxy.testnet.concordium.com/v1/ip_info"
    )]
    wp_url:            url::Url,
    /// Location of the seed phrase.
    #[clap(
        long,
        help = "Path to the seed phrase file. Specify either this or --id-cred-sec, --prf-key.",
        conflicts_with_all = ["prf_key", "id_cred_sec"],
        required_unless_present_all = ["prf_key", "id_cred_sec"]
    )]
    concordium_wallet: Option<std::path::PathBuf>,
    #[clap(long = "ip-index", help = "Identity of the identity provider.")]
    ip_index:          u32,
    #[clap(
        long,
        help = "Hex encoded id credential secret. Specify either this or --concordium-wallet.",
        required_unless_present = "concordium_wallet",
        requires_all = ["prf_key"]
    )]
    id_cred_sec:       Option<String>,
    #[clap(
        long,
        help = "Hex encoded PRF key. Specify either this or --concordium-wallet.",
        required_unless_present = "concordium_wallet",
        requires_all = ["id_cred_sec"]
    )]
    prf_key:           Option<String>,
    #[clap(long, help = "Network to recover on.", default_value = "testnet")]
    network:           key_derivation::Net,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app: Api = Api::parse();

    let concordium_client = {
        // Use TLS if the URI scheme is HTTPS.
        // This uses whatever system certificates have been installed as trusted roots.
        let endpoint = if app
            .api
            .uri()
            .scheme()
            .map_or(false, |x| x == &v2::Scheme::HTTPS)
        {
            app.api
                .tls_config(ClientTlsConfig::new())
                .context("Unable to construct TLS configuration for the Concordium API.")?
        } else {
            app.api
        };
        let ep = endpoint
            .timeout(std::time::Duration::from_secs(
                app.concordium_request_timeout,
            ))
            .connect_timeout(std::time::Duration::from_secs(10));
        concordium::v2::Client::new(ep)
            .await
            .context("Unable to connect Concordium node.")?
    };

    match app.command {
        Command::GenerateSecrets(args) => generate_secrets(concordium_client, args).await,
        Command::RecoverIdentity(args) => recover_identity(concordium_client, args).await,
    }
}

/// When finding all accounts for an identity, we stop after this many failures,
/// i.e. if we have not found any accounts for this many consecutive indices.
const MAX_ACCOUNT_FAILURES: u8 = 20;
/// When finding all identities for a seedphrase, we stop after this many
/// failures, i.e. if we have failed to find accounts for a candidate identity
/// this many times.
const MAX_IDENTITY_FAILURES: u8 = 20;

async fn generate_secrets(
    mut concordium_client: v2::Client,
    generate_args: GenerateSecretsArgs,
) -> anyhow::Result<()> {
    let seed_phrase = std::fs::read_to_string(generate_args.concordium_wallet)?;
    let words = seed_phrase.split_ascii_whitespace().collect::<Vec<_>>();
    let wallet = ConcordiumHdWallet::from_words(&words, generate_args.network)?;

    let crypto_params = concordium_client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;

    let ip_index = generate_args.ip_index;

    // Loop through all identities until we find one with an account.
    let id_index = generate_args.id_index;
    let mut acc_fail_count = 0;
    let mut accs_found = 0;
    for acc_index in 0u8..=255 {
        let reg_id = {
            // This needs to be in a separate scope to avoid keeping prf_key across an await
            // boundary
            let prf_key = wallet
                .get_prf_key(ip_index, id_index)
                .context("Failed to get PRF key.")?;
            let reg_id = prf_key
                .prf(crypto_params.elgamal_generator(), acc_index)
                .context("Failed to compute PRF.")?;
            AccountIdentifier::CredId(CredentialRegistrationID::new(reg_id))
        };
        match concordium_client
            .get_account_info(&reg_id, v2::BlockIdentifier::LastFinal)
            .await
        {
            Ok(info) => {
                println!(
                    "Account with address {} found at index {acc_index}.",
                    info.response.account_address
                );
                accs_found += 1;
            }
            Err(e) if e.is_not_found() => {
                acc_fail_count += 1;
                if acc_fail_count > MAX_ACCOUNT_FAILURES {
                    break;
                }
            }
            Err(e) => bail!("Cannot query the node: {e}"),
        }
    }

    if accs_found == 0 {
        println!("No accounts were found for the supplied indices.");
    }

    let prf_key = wallet
        .get_prf_key(ip_index, id_index)
        .context("Failed to get PRF key.")?;
    let id_cred_scalar = wallet
        .get_id_cred_sec(ip_index, id_index)
        .context("Could not get idCredSec")?;
    let id_cred_sec: PedersenValue<ArCurve> = PedersenValue::new(id_cred_scalar);
    println!("prf-key: {}", base16_encode_string(&prf_key));
    println!("id-cred-sec: {}", base16_encode_string(&id_cred_sec));

    Ok(())
}

async fn recover_identity(
    mut concordium_client: v2::Client,
    recovery_args: RecoverIdentityArgs,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let ids = client
        .get(recovery_args.wp_url)
        .send()
        .await?
        .json::<Vec<WpIpInfos>>()
        .await?;

    let Some(id) = ids.into_iter().find(|x| x.ip_info.ip_identity == recovery_args.ip_index.into()) else {
        anyhow::bail!("Identity provider not found.");
    };
    println!("Using identity provider {}", id.ip_info.ip_description.name);

    let crypto_params = concordium_client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;

    if let Some(concordium_wallet) = recovery_args.concordium_wallet {
        recover_from_wallet(
            concordium_client,
            client,
            id,
            crypto_params,
            concordium_wallet,
            recovery_args.network,
        )
        .await
    } else {
        let prf_key: PrfKey =
            base16_decode_string(&recovery_args.prf_key.context("Missing prf_key")?)?;
        let id_cred_sec: PedersenValue<ArCurve> =
            base16_decode_string(&recovery_args.id_cred_sec.context("Missing prf_key")?)?;

        recover_from_secrets(
            &mut concordium_client,
            &client,
            &id,
            &crypto_params,
            prf_key,
            id_cred_sec,
            "recovered-from-id-cred-sec",
        )
        .await
        .context("Could not recover identity")
    }
}

async fn recover_from_wallet(
    mut concordium_client: v2::Client,
    client: reqwest::Client,
    id: WpIpInfos,
    crypto_params: GlobalContext<ArCurve>,
    concordium_wallet: std::path::PathBuf,
    network: key_derivation::Net,
) -> anyhow::Result<()> {
    let seed_phrase = std::fs::read_to_string(concordium_wallet)?;
    let words = seed_phrase.split_ascii_whitespace().collect::<Vec<_>>();
    let wallet = ConcordiumHdWallet::from_words(&words, network)?;

    // Try to find all identities for this wallet.
    let mut failure_count = 0;
    let mut success_count = 0;
    for id_index in 0.. {
        let id_cred_sec = wallet.get_id_cred_sec(id.ip_info.ip_identity.0, id_index)?;
        let id_cred_sec = PedersenValue::new(id_cred_sec);
        let prf_key = wallet.get_prf_key(id.ip_info.ip_identity.0, id_index)?;
        let result = recover_from_secrets(
            &mut concordium_client,
            &client,
            &id,
            &crypto_params,
            prf_key,
            id_cred_sec,
            id_index,
        )
        .await;

        if result.is_ok() {
            failure_count = 0;
            success_count += 1;
        } else {
            failure_count += 1;
        }
        if failure_count > MAX_IDENTITY_FAILURES {
            break;
        }
    }

    if success_count == 0 {
        Err(anyhow::anyhow!("Failed to find an identity for wallet."))
    } else {
        Ok(())
    }
}

async fn recover_from_secrets(
    concordium_client: &mut v2::Client,
    client: &reqwest::Client,
    id: &WpIpInfos,
    crypto_params: &GlobalContext<ArCurve>,
    prf_key: PrfKey,
    id_cred_sec: PedersenValue<ArCurve>,
    id_description: impl std::fmt::Display, /* description of the identity, e.g., index of the
                                             * identity. */
) -> anyhow::Result<()> {
    let request = generate_id_recovery_request(
        &id.ip_info,
        crypto_params,
        &id_cred_sec,
        chrono::Utc::now().timestamp() as u64,
    )
    .context("Unable to construct recovery request")?;
    let response = client
        .get(id.metadata.recovery_start.clone())
        .query(&[(
            "state",
            serde_json::to_string(&RecoveryRequestData {
                id_recovery_request: Versioned::new(VERSION_0, request),
            })
            .unwrap(),
        )])
        .send()
        .await?;

    if !response.status().is_success() {
        bail!("Recovery request failed: {}", response.text().await?);
    }

    let id_object: Versioned<IdentityObjectV1<IpPairing, ArCurve, AttributeKind>> =
        response.json().await?;
    let id_descr = id_description.to_string();
    std::fs::write(
        format!("{}-{id_description}.json", id.ip_info.ip_identity.0),
        serde_json::to_string_pretty(&serde_json::json!({
            "ipInfo": &id.ip_info,
            "idObject": id_object.value
        }))?,
    )?;
    println!("Got identity object for index {id_descr}.");

    // Print all accounts for this identity.
    let mut acc_fail_count = 0;
    for acc_idx in 0u8..=id_object.value.alist.max_accounts {
        let reg_id = prf_key.prf(crypto_params.elgamal_generator(), acc_idx)?;
        let address = AccountIdentifier::CredId(CredentialRegistrationID::new(reg_id));
        match concordium_client
            .get_account_info(&address, BlockIdentifier::LastFinal)
            .await
        {
            Ok(info) => println!(
                "Account with address {} found at index {acc_idx}",
                info.response.account_address
            ),
            Err(e) if e.is_not_found() => {
                acc_fail_count += 1;
                if acc_fail_count > MAX_ACCOUNT_FAILURES {
                    break;
                }
            }
            Err(e) => anyhow::bail!("Cannot query the node: {e}"),
        }
    }

    Ok(())
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequestData {
    id_recovery_request: Versioned<IdRecoveryRequest<ArCurve>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct WpIpInfos {
    ip_info:  IpInfo<IpPairing>,
    metadata: RecoveryStart,
}

#[derive(serde::Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct RecoveryStart {
    recovery_start: url::Url,
}

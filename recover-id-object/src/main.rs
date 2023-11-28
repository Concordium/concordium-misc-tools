use anyhow::{bail, Context};
use clap::{Args, Parser, Subcommand};
use concordium::{
    common::{base16_decode_string, base16_encode_string, Versioned, VERSION_0},
    id::{
        account_holder::generate_id_recovery_request,
        constants::{ArCurve, AttributeKind, IpPairing},
        pedersen_commitment::Value as PedersenValue,
        types::{
            account_address_from_registration_id, GlobalContext, IdRecoveryRequest,
            IdentityObjectV1, IpInfo,
        },
    },
    v2,
    v2::BlockIdentifier,
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
        default_value = "http://localhost:20000"
    )]
    api: concordium::v2::Endpoint,
    /// Request timeout for Concordium node requests.
    #[clap(
        long,
        help = "Timeout for requests to the Concordium node.",
        default_value = "10"
    )]
    concordium_request_timeout: u64,
    #[clap(long = "ip-index", help = "Identity of the identity provider.")]
    ip_index: u32,
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
}

#[derive(Debug, Args)]
// #[clap(group = ArgGroup::new("recovery-secrets").multiple(true))]
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
        help = "Path to the seed phrase file. Specify either this or --id-cred-sec, --prf-key, and --id-index.",
        conflicts_with_all = ["prf_key", "id_cred_sec", "id_index"],
        required_unless_present_all = ["prf_key", "id_cred_sec", "id_index"]
    )]
    concordium_wallet: Option<std::path::PathBuf>,
    #[clap(
        long,
        help = "Hex encoded id credential secret. Specify either this or --concordium-wallet.",
        required_unless_present = "concordium_wallet",
        requires_all = ["prf_key", "id_index"]
    )]
    id_cred_sec:       Option<String>,
    #[clap(
        long,
        help = "Hex encoded PRF key. Specify either this or --concordium-wallet.",
        required_unless_present = "concordium_wallet",
        requires_all = ["id_cred_sec", "id_index"]
    )]
    prf_key:           Option<String>,
    #[clap(
        long,
        help = "Identity index of account to recover. Specify either this or --concordium-wallet.",
        required_unless_present = "concordium_wallet",
        requires_all = ["id_cred_sec", "prf_key"]
    )]
    id_index:          Option<u32>,
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
            .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
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
        Command::GenerateSecrets(args) => {
            generate_secrets(concordium_client, app.ip_index, args).await
        }
        Command::RecoverIdentity(args) => {
            recover_identity(concordium_client, app.ip_index, args).await
        }
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
    ip_index: u32,
    generate_args: GenerateSecretsArgs,
) -> anyhow::Result<()> {
    let seed_phrase = std::fs::read_to_string(generate_args.concordium_wallet)?;
    let words = seed_phrase.split_ascii_whitespace().collect::<Vec<_>>();
    let wallet = ConcordiumHdWallet::from_words(&words, key_derivation::Net::Testnet);

    let crypto_params = concordium_client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;

    // Loop through all identities until we find one with an account.
    let mut id_index = 0;
    let mut id_fail_count = 0;
    'id_loop: loop {
        let mut acc_fail_count = 0;
        for acc_index in 0u8..=255 {
            let address = {
                // This needs to be in a separate scope to avoid keeping prf_key across an await
                // boundary
                let prf_key = wallet
                    .get_prf_key(ip_index, id_index)
                    .context("Failed to get PRF key.")?;
                let reg_id = prf_key
                    .prf(crypto_params.elgamal_generator(), acc_index)
                    .context("Failed to compute PRF.")?;
                account_address_from_registration_id(&reg_id)
            };
            match concordium_client
                .get_account_info(&address.into(), v2::BlockIdentifier::LastFinal)
                .await
            {
                Ok(_) => {
                    println!("Account with address {address} found.");
                    break 'id_loop;
                }
                Err(e) if e.is_not_found() => {
                    acc_fail_count += 1;
                    if acc_fail_count > MAX_ACCOUNT_FAILURES {
                        id_fail_count += 1;
                        if id_fail_count > MAX_IDENTITY_FAILURES {
                            bail!("Failed to find an identity.");
                        }
                        break;
                    }
                }
                Err(e) => bail!("Cannot query the node: {e}"),
            }
        }
        id_index += 1;
    }

    println!("id-index: {id_index}");
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
    ip_index: u32,
    recovery_args: RecoverIdentityArgs,
) -> anyhow::Result<()> {
    let client = reqwest::Client::new();
    let ids = client
        .get(recovery_args.wp_url)
        .send()
        .await?
        .json::<Vec<WpIpInfos>>()
        .await?;

    let Some(id) = ids.into_iter().find(|x| x.ip_info.ip_identity == ip_index.into()) else {
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
        )
        .await
    } else {
        let prf_key: PrfKey =
            base16_decode_string(&recovery_args.prf_key.context("Missing prf_key")?)?;
        let id_cred_sec: PedersenValue<ArCurve> =
            base16_decode_string(&recovery_args.id_cred_sec.context("Missing prf_key")?)?;
        let id_index = recovery_args.id_index.context("Missing id_index")?;

        recover_from_secrets(
            &mut concordium_client,
            &client,
            &id,
            &crypto_params,
            prf_key,
            id_cred_sec,
            id_index,
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
) -> anyhow::Result<()> {
    let seed_phrase = std::fs::read_to_string(concordium_wallet)?;
    let words = seed_phrase.split_ascii_whitespace().collect::<Vec<_>>();
    let wallet = ConcordiumHdWallet::from_words(&words, key_derivation::Net::Testnet);

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
    id_index: u32,
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
    std::fs::write(
        format!("{}-{id_index}.json", id.ip_info.ip_identity.0),
        serde_json::to_string_pretty(&serde_json::json!({
            "identityIndex": id_index,
            "ipInfo": &id.ip_info,
            "idObject": id_object.value
        }))?,
    )?;
    println!("Got identity object for index {id_index}.");

    // Print all accounts for this identity.
    let mut acc_fail_count = 0;
    for acc_idx in 0u8..=id_object.value.alist.max_accounts {
        let reg_id = prf_key.prf(crypto_params.elgamal_generator(), acc_idx)?;
        let address = account_address_from_registration_id(&reg_id);
        match concordium_client
            .get_account_info(&address.into(), BlockIdentifier::LastFinal)
            .await
        {
            Ok(_) => println!("Account with address {address} found."),
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

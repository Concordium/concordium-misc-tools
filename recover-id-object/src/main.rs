use anyhow::Context;
use clap::Parser;
use concordium::{
    common::{Versioned, VERSION_0},
    id::{
        account_holder::generate_id_recovery_request,
        constants::{ArCurve, AttributeKind, IpPairing},
        types::{
            account_address_from_registration_id, IdRecoveryRequest, IdentityObjectV1, IpInfo,
        },
    },
    v2::BlockIdentifier,
};
use concordium_rust_sdk as concordium;
use key_derivation::ConcordiumHdWallet;
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
    /// Location of the seed phrase.
    #[clap(long, help = "Path to the seed phrase file.")]
    concordium_wallet: std::path::PathBuf,
    /// Recovery URL start.
    #[clap(
        long = "ip-info-url",
        help = "Identity recovery URL",
        default_value = "http://wallet-proxy.testnet.concordium.com/v1/ip_info"
    )]
    wp_url: url::Url,
    #[clap(long = "ip-index", help = "Identity of the identity provider.")]
    ip_index: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app: Api = Api::parse();

    let mut concordium_client = {
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

    let seed_phrase = std::fs::read_to_string(app.concordium_wallet)?;
    let words = seed_phrase.split_ascii_whitespace().collect::<Vec<_>>();
    let wallet = ConcordiumHdWallet::from_words(&words, key_derivation::Net::Testnet);

    let client = reqwest::Client::new();

    let ids = client
        .get(app.wp_url)
        .send()
        .await?
        .json::<Vec<WpIpInfos>>()
        .await?;

    let Some(id) = ids.into_iter().find(|x| x.ip_info.ip_identity == app.ip_index.into()) else {
        anyhow::bail!("Identity provider not found.")
    };
    println!("Using identity provider {}", id.ip_info.ip_description.name);

    let crypto_params = concordium_client
        .get_cryptographic_parameters(BlockIdentifier::LastFinal)
        .await?
        .response;
    let mut failure_count = 0;
    for idx in 0.. {
        let request = generate_id_recovery_request(
            &id.ip_info,
            &crypto_params,
            &concordium::id::pedersen_commitment::Value::new(
                wallet.get_id_cred_sec(app.ip_index, idx)?,
            ),
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
        if response.status().is_success() {
            let id_object = response
                .json::<Versioned<IdentityObjectV1<IpPairing, ArCurve, AttributeKind>>>()
                .await?;
            std::fs::write(
                format!("{}-{idx}.json", app.ip_index),
                serde_json::to_string_pretty(&serde_json::json!({
                    "identityIndex": idx,
                    "ipInfo": &id.ip_info,
                    "idObject": id_object.value
                }))?,
            )?;
            println!("Got identity object for index {idx}.");
            let mut acc_fail_count = 0;
            for acc_idx in 0u8..=255 {
                let prf_key = wallet.get_prf_key(app.ip_index, idx)?;
                let reg_id = prf_key.prf(crypto_params.elgamal_generator(), acc_idx)?;
                let address = account_address_from_registration_id(&reg_id);
                match concordium_client
                    .get_account_info(&address.into(), BlockIdentifier::LastFinal)
                    .await
                {
                    Ok(_) => println!("Account with address {address} found."),
                    Err(e) if e.is_not_found() => {
                        acc_fail_count += 1;
                        if acc_fail_count > 5 {
                            break;
                        }
                    }
                    Err(e) => anyhow::bail!("Cannot query the node: {e}"),
                }
            }
            failure_count = 0;
        } else {
            failure_count += 1;
        }
        if failure_count > 5 {
            break;
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

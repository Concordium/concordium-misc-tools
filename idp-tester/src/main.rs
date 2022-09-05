use std::{collections::BTreeMap, num::NonZeroU32};

use anyhow::Context;
use clap::Parser;
use concordium_rust_sdk::{
    common::types::KeyIndex,
    id::{self, dodis_yampolskiy_prf},
};
use rand::SeedableRng;

#[derive(clap::Parser, Debug)]
#[clap(arg_required_else_help(true), propagate_version = true)]
#[clap(version, author)]
struct App {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    #[clap(name = "get")]
    Get(Get),
    #[clap(name = "recover")]
    Recover(Recover),
}

#[derive(clap::Args, Debug)]
struct Get {
    #[clap(long = "issuance-start", help = "URL where issuance starts.")]
    url:       reqwest::Url,
    #[clap(
        long = "node",
        help = "Node to connect to get identity providers and anonymity revokers."
    )]
    node:      concordium_rust_sdk::endpoints::Endpoint,
    #[clap(long = "token", help = "Token for the rpc interface.", default_value = "rpcadmin")]
    token:     String,
    #[clap(long = "idp", help = "Identity of the identity provider to use.")]
    idp:       u32,
    #[clap(long = "ar", help = "Anonymity revokers to use.")]
    ars:       Vec<NonZeroU32>,
    #[clap(long = "threshold", help = "Anonymity revoker threshold to use.")]
    threshold: u8,
    #[clap(long = "v0", help = "Use v0 request instead of the default v1.")]
    v0:        bool,
    #[clap(long = "seed", help = "Use a deterministic seed when generating the request.")]
    seed:      u64,
}

#[derive(clap::Args, Debug)]
struct Recover {
    #[clap(long = "recovery-url", help = "URL where recovery.")]
    url:   reqwest::Url,
    #[clap(long = "node", help = "Node to connect to get identity providers.")]
    node:  concordium_rust_sdk::endpoints::Endpoint,
    #[clap(long = "token", help = "Token for the rpc interface.", default_value = "rpcadmin")]
    token: String,
    #[clap(long = "idp", help = "Identity of the identity provider to use.")]
    idp:   u32,
    #[clap(long = "seed", help = "Use a deterministic seed when generating the request.")]
    seed:  u64,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();
    match app.command {
        Command::Get(get) => handle_get(get).await,
        Command::Recover(recover) => handle_recover(recover).await,
    }
}

async fn handle_recover(app: Recover) -> anyhow::Result<()> {
    let mut csprng = rand::rngs::StdRng::seed_from_u64(app.seed);

    // NB: Important to use the csprng here for determinism
    let prf_key = dodis_yampolskiy_prf::SecretKey::<id::constants::ArCurve>::generate(&mut csprng);

    let chi = id::types::CredentialHolderInfo::<id::constants::ArCurve> {
        id_cred: id::types::IdCredentials::generate(&mut csprng),
    };

    let mut client = concordium_rust_sdk::endpoints::Client::connect(app.node, &app.token).await?;

    let ci = client.get_consensus_status().await?;

    let idps = client.get_identity_providers(&ci.last_finalized_block).await?;

    let ip_info = idps
        .into_iter()
        .find(|x| x.ip_identity.0 == app.idp)
        .context("Identity provider not found.")?;

    let global_context = client.get_cryptographic_parameters(&ci.last_finalized_block).await?;

    println!("Using idp {}.", ip_info.ip_description.name);

    let request = id::account_holder::generate_id_recovery_request(
        &ip_info,
        &global_context,
        &chi.id_cred.id_cred_sec,
        chrono::Utc::now().timestamp() as u64,
    )
    .context("Cannot generate request.")?;

    let request = serde_json::json!({
        "idRecoveryRequest": serde_json::json!({
            "v": 0,
            "value": request
        })
    });
    println!("{}", serde_json::to_string_pretty(&request).unwrap());
    let mut request_url = app.url.clone();
    request_url.query_pairs_mut().append_pair("state", serde_json::to_string(&request)?.as_str());
    let r = reqwest::ClientBuilder::new().redirect(reqwest::redirect::Policy::none()).build()?;
    let r = r.get(request_url).send().await?;
    println!("Used URL = {}", r.url());
    if r.status().is_success() {
        println!("{:?}", r);
        println!("{}", serde_json::to_string_pretty(&r.json::<serde_json::Value>().await?)?);
    } else {
        println!("{}", r.status());
        for (hn, hv) in r.headers() {
            println!("{} = {:?}", hn, hv.to_str());
        }
        println!("{:?}", r.text().await);
    }
    Ok(())
}

async fn handle_get(app: Get) -> anyhow::Result<()> {
    let mut csprng = rand::rngs::StdRng::seed_from_u64(app.seed);

    let prf_key = dodis_yampolskiy_prf::SecretKey::generate(&mut csprng);

    let chi = id::types::CredentialHolderInfo::<id::constants::ArCurve> {
        id_cred: id::types::IdCredentials::generate(&mut csprng),
    };

    let aci = id::types::AccCredentialInfo {
        cred_holder_info: chi,
        prf_key,
    };
    // TODO: Export ps_sig from the SDK.
    let randomness = ps_sig::SigRetrievalRandomness::generate_non_zero(&mut csprng);
    let id_use_data = id::types::IdObjectUseData {
        aci,
        randomness,
    };

    let mut client = concordium_rust_sdk::endpoints::Client::connect(app.node, &app.token).await?;

    let ci = client.get_consensus_status().await?;

    let idps = client.get_identity_providers(&ci.last_finalized_block).await?;
    let ars = client.get_anonymity_revokers(&ci.last_finalized_block).await?;

    let ip_info = idps
        .into_iter()
        .find(|x| x.ip_identity.0 == app.idp)
        .context("Identity provider not found.")?;

    let ars_infos = ars
        .into_iter()
        .filter_map(|ar| {
            if app.ars.iter().any(|i| u32::from(*i) == u32::from(ar.ar_identity)) {
                Some((ar.ar_identity, ar))
            } else {
                None
            }
        })
        .collect::<BTreeMap<_, _>>();

    let global_context = client.get_cryptographic_parameters(&ci.last_finalized_block).await?;

    println!("Using idp {}.", ip_info.ip_description.name);

    let context = id::types::IpContext::new(&ip_info, &ars_infos, &global_context);
    let request = if app.v0 {
        // Generating account data for the initial account
        let mut keys = std::collections::BTreeMap::new();
        keys.insert(
            KeyIndex(0),
            concordium_rust_sdk::common::types::KeyPair::from(ed25519_dalek::Keypair::generate(
                &mut csprng,
            )),
        );

        let initial_acc_data = id::types::InitialAccountData {
            keys,
            threshold: id::types::SignatureThreshold(1),
        };
        let (request, randomness) = id::account_holder::generate_pio(
            &context,
            app.threshold.try_into().map_err(|()| anyhow::anyhow!("Unsupported threshold."))?,
            &id_use_data,
            &initial_acc_data,
        )
        .context("Cannot generate request.")?;
        serde_json::json!({
            "idObjectRequest": serde_json::json!({
                "v": 0,
                "value": request
            })
        })
    } else {
        let (request, randomness) = id::account_holder::generate_pio_v1(
            &context,
            app.threshold.try_into().map_err(|()| anyhow::anyhow!("Unsupported threshold."))?,
            &id_use_data,
        )
        .context("Cannot generate request.")?;

        serde_json::json!({
            "idObjectRequest": serde_json::json!({
                "v": 0,
                "value": request
            })
        })
    };
    let mut request_url = app.url.clone();
    request_url
        .query_pairs_mut()
        .append_pair("state", serde_json::to_string(&request)?.as_str())
        .append_pair("redirect_uri", "REDIRECT")
        .append_pair("response_type", "code")
        .append_pair("scope", "identity")
        .append_pair("test_flow", "1");
    let r = reqwest::ClientBuilder::new().redirect(reqwest::redirect::Policy::none()).build()?;
    let r = r.get(request_url).send().await?;
    if r.status().is_redirection() {
        let url = app.url;
        let redirect_url = url.join(r.headers().get("location").unwrap().to_str().unwrap())?;
        println!("Go to {}", redirect_url);
    } else {
        println!("{}", r.status());
        for (hn, hv) in r.headers() {
            println!("{} = {:?}", hn, hv.to_str());
        }
        println!("{:?}", r.text().await);
    }
    Ok(())
}

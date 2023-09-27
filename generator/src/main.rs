use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use concordium_rust_sdk::{
    common::types::Amount,
    endpoints::Endpoint,
    types::WalletAccount,
    v2::{self},
};
use generator::{
    generate_transactions, CcdGenerator, CommonArgs, MintCis2Generator,
    RegisterCredentialsGenerator, TransferCis2Generator, WccdGenerator,
};
use std::{path::PathBuf, str::FromStr};

mod generator;

#[derive(Debug, Clone, Copy)]
enum Mode {
    Random,
    Every(usize),
}

impl FromStr for Mode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "random" => Ok(Self::Random),
            s => Ok(Self::Every(s.parse()?)),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Send CCD to a list of receivers.
    Ccd(CcdArgs),
    /// Mint CIS-2 NFT tokens.
    MintNfts,
    /// Transfer CIS-2 tokens to a list of receivers.
    TransferCis2(TransferCis2Args),
    /// Wrap, unwrap, and transfer WCCD tokens. First, wCCD are minted for every
    /// account on the chain and then 1 wCCD is alternately wrapped,
    /// transferred, and unwrapped.
    Wccd,
    /// Register Web3 ID credentials.
    RegisterCredentials,
}

#[derive(Debug, Args)]
pub struct CcdArgs {
    #[arg(long = "receivers", help = "Path to file containing receivers.")]
    receivers: Option<PathBuf>,
    #[clap(
        long = "amount",
        help = "CCD amount to send in each transaction",
        default_value = "0"
    )]
    amount:    Amount,
    #[clap(
        long = "mode",
        help = "If set this provides the mode when selecting accounts. It can either be `random` \
                or a non-negative integer. If it is an integer then the set of receivers is \
                partitioned based on baker id into the given amount of chunks."
    )]
    mode:      Option<Mode>,
}

#[derive(Debug, Args)]
pub struct TransferCis2Args {
    #[arg(long = "receivers", help = "Path to file containing receivers.")]
    receivers: Option<PathBuf>,
}

#[derive(clap::Parser, Debug)]
#[clap(author, version, about)]
/// A transaction generator used for testing performance of the chain.
struct App {
    #[clap(
        long = "node",
        help = "GRPC interface of the node.",
        default_value = "http://localhost:20000"
    )]
    endpoint: Endpoint,
    #[clap(long = "sender")]
    account:  PathBuf,
    #[clap(long = "tps")]
    tps:      u16,
    #[clap(
        long = "expiry",
        help = "Expiry of transactions in seconds.",
        default_value = "7200"
    )]
    expiry:   u32,

    #[command(subcommand)]
    command: Command,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();

    let client = {
        // Use TLS if the URI scheme is HTTPS.
        // This uses whatever system certificates have been installed as trusted roots.
        let endpoint = if app
            .endpoint
            .uri()
            .scheme()
            .map_or(false, |x| x == &http::uri::Scheme::HTTPS)
        {
            app.endpoint
                .tls_config(tonic::transport::channel::ClientTlsConfig::new())
                .context("Unable to construct TLS configuration for the Concordium API.")?
        } else {
            app.endpoint
        };
        let ep = endpoint.connect_timeout(std::time::Duration::from_secs(10));
        v2::Client::new(ep)
            .await
            .context("Unable to connect Concordium node.")?
    };

    let keys: WalletAccount =
        WalletAccount::from_json_file(app.account).context("Could not parse the keys file.")?;
    let args = CommonArgs {
        keys,
        expiry: app.expiry,
    };

    match app.command {
        Command::Ccd(ccd_args) => {
            let generator = CcdGenerator::instantiate(client.clone(), args, ccd_args).await?;
            generate_transactions(client, generator, app.tps).await
        }
        Command::MintNfts => {
            let generator = MintCis2Generator::instantiate(client.clone(), args).await?;
            generate_transactions(client, generator, app.tps).await
        }
        Command::TransferCis2(transfer_cis2_args) => {
            let generator =
                TransferCis2Generator::instantiate(client.clone(), args, transfer_cis2_args)
                    .await?;
            generate_transactions(client, generator, app.tps).await
        }
        Command::Wccd => {
            let generator = WccdGenerator::instantiate(client.clone(), args).await?;
            generate_transactions(client, generator, app.tps).await
        }
        Command::RegisterCredentials => {
            let generator = RegisterCredentialsGenerator::instantiate(client.clone(), args).await?;
            generate_transactions(client, generator, app.tps).await
        }
    }
}

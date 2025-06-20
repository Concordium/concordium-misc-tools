use anyhow::Context;
use clap::{Parser, Subcommand};
use concordium_rust_sdk::{endpoints::Endpoint, types::WalletAccount, v2};
use generator::{
    generate_transactions, CcdGenerator, CommonArgs, MintCis2Generator,
    RegisterCredentialsGenerator, RegisterDataGenerator, TransferCis2Generator, WccdGenerator,
};
use std::path::PathBuf;

mod generator;

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
    #[clap(long = "sender", help = "Path to file containing sender keys.")]
    account:  PathBuf,
    #[clap(
        long = "tps",
        help = "Transactions to send per second.",
        default_value = "1"
    )]
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

#[derive(Debug, Subcommand)]
enum Command {
    /// Send CCD to a list of receivers.
    Ccd(generator::CcdArgs),
    /// Mint CIS-2 NFT tokens.
    MintNfts,
    /// Transfer CIS-2 tokens to a list of receivers.
    TransferCis2(generator::TransferCis2Args),
    /// Wrap, unwrap, and transfer WCCD tokens. First, wCCD are minted for every
    /// account on the chain and then 1 wCCD is alternately wrapped,
    /// transferred, and unwrapped.
    Wccd,
    /// Register Web3 ID credentials.
    RegisterCredentials,
    /// Register data
    RegisterData(generator::RegisterDataArgs),
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let app = App::parse();

    let client = {
        // Use TLS if the URI scheme is HTTPS.
        // This uses whatever system certificates have been installed as trusted roots.
        let endpoint = if app.endpoint.uri().scheme() == Some(&v2::Scheme::HTTPS) {
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
        Command::RegisterData(register_data_args) => {
            let generator =
                RegisterDataGenerator::instantiate(client.clone(), args, register_data_args)
                    .await?;
            generate_transactions(client, generator, app.tps).await
        }
    }
}

use anyhow::Context;
use clap::{Args, Parser, Subcommand};
use concordium_rust_sdk::{
    cis2::{AdditionalData, Receiver, TokenAmount, TokenId, Transfer, TransferParams},
    common::{
        types::{Amount, TransactionTime},
        Deserial,
    },
    contract_client::MetadataUrl,
    endpoints::Endpoint,
    id::types::AccountAddress,
    smart_contracts::common as concordium_std,
    types::{
        smart_contracts::{OwnedContractName, OwnedParameter, OwnedReceiveName, WasmModule},
        transactions::{
            send, AccountTransaction, BlockItem, EncodedPayload, InitContractPayload,
            UpdateContractPayload,
        },
        Address, ContractAddress, Energy, NodeDetails, Nonce, WalletAccount,
    },
    v2::{self, BlockIdentifier},
};
use futures::TryStreamExt;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{collections, collections::BTreeMap, io::Cursor, path::PathBuf, str::FromStr, sync::Arc};

const MINT_CIS2_MODULE: &'static [u8] = include_bytes!("../resources/cis2_nft.wasm.v1");
const TRANSFER_CIS2_MODULE: &'static [u8] = include_bytes!("../resources/cis2_multi.wasm.v1");

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
}

#[derive(Debug, Args)]
struct CcdArgs {
    #[arg(long = "receivers")]
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
struct TransferCis2Args {
    #[arg(long = "receivers")]
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

#[derive(Clone)]
struct CommonArgs {
    client: v2::Client,
    keys:   Arc<WalletAccount>,
    tps:    u16,
    expiry: u32,
}

struct ContractDeploymentInfo {
    module:      &'static [u8],
    name:        &'static str,
    init_energy: Energy,
}

impl CommonArgs {
    async fn deploy_and_init_contract(
        &mut self,
        info: ContractDeploymentInfo,
        nonce: &mut Nonce,
    ) -> anyhow::Result<ContractAddress> {
        println!("Deploying and initializing contract...");

        let expiry: TransactionTime = TransactionTime::seconds_after(self.expiry);
        let module = WasmModule::deserial(&mut Cursor::new(info.module))?;
        let mod_ref = module.get_module_ref();
        let deploy_tx = send::deploy_module(&*self.keys, self.keys.address, *nonce, expiry, module);
        nonce.next_mut();

        let item = BlockItem::AccountTransaction(deploy_tx);
        self.client.send_block_item(&item).await?;

        let payload = InitContractPayload {
            amount: Amount::zero(),
            mod_ref,
            init_name: OwnedContractName::new(info.name.into())?,
            param: OwnedParameter::empty(),
        };
        let init_tx = send::init_contract(
            &*self.keys,
            self.keys.address,
            *nonce,
            expiry,
            payload,
            info.init_energy,
        );
        nonce.next_mut();

        let item = BlockItem::AccountTransaction(init_tx);
        let transaction_hash = self.client.send_block_item(&item).await?;
        let (_, summary) = self.client.wait_until_finalized(&transaction_hash).await?;
        anyhow::ensure!(summary.is_success(), "Contract init transaction failed.");
        println!(
            "Contract init transaction finalized (hash = {transaction_hash}, energy = {}).",
            summary.energy_cost,
        );

        summary
            .contract_init()
            .context("Transaction was not a contract init")
            .map(|init| init.address)
    }
}

trait Generate {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>>;
}

async fn generate_transactions(
    mut args: CommonArgs,
    mut generator: impl Generate + Send + 'static,
) -> anyhow::Result<()> {
    // Create a channel between the task signing and the task sending transactions.
    let (sender, mut rx) = tokio::sync::mpsc::channel(100);

    // A task that will generate and sign transactions.
    let generator_task = async move {
        loop {
            let tx = generator.generate();
            sender.send(tx).await.expect("receiver exists");
        }
    };
    // Spawn it to run in the background.
    tokio::spawn(generator_task);

    let mut interval = tokio::time::interval(tokio::time::Duration::from_micros(
        1_000_000 / u64::from(args.tps),
    ));
    loop {
        interval.tick().await;
        if let Some(tx) = rx.recv().await.transpose()? {
            let nonce = tx.header.nonce;
            let energy = tx.header.energy_amount;
            let item = BlockItem::AccountTransaction(tx);
            let transaction_hash = args.client.send_block_item(&item).await?;
            println!(
                "{}: Transaction {} submitted (nonce = {nonce}, energy = {energy}).",
                chrono::Utc::now(),
                transaction_hash,
            );
        } else {
            break Ok(());
        }
    }
}

struct CcdGenerator {
    args:     CommonArgs,
    amount:   Amount,
    accounts: Vec<AccountAddress>,
    random:   bool,
    rng:      StdRng,
    count:    usize,
    nonce:    Nonce,
}

impl CcdGenerator {
    async fn instantiate(mut args: CommonArgs, ccd_args: CcdArgs) -> anyhow::Result<Self> {
        let accounts: Vec<AccountAddress> = match ccd_args.receivers {
            None => {
                args.client
                    .get_account_list(BlockIdentifier::LastFinal)
                    .await
                    .context("Could not obtain a list of accounts.")?
                    .response
                    .try_collect()
                    .await?
            }
            Some(receivers) => serde_json::from_str(
                &std::fs::read_to_string(receivers)
                    .context("Could not read the receivers file.")?,
            )
            .context("Could not parse the receivers file.")?,
        };
        anyhow::ensure!(!accounts.is_empty(), "List of receivers must not be empty.");

        let (random, accounts) = match ccd_args.mode {
            Some(Mode::Random) => (true, accounts),
            Some(Mode::Every(n)) if n > 0 => {
                let ni = args.client.get_node_info().await?;
                if let NodeDetails::Node(nd) = ni.details {
                    let baker = nd
                        .baker()
                        .context("Node is not a baker but integer mode is required.")?;
                    let step = accounts.len() / n;
                    let start = baker.id.index as usize % n;
                    let end = std::cmp::min(accounts.len(), (start + 1) * step);
                    (false, accounts[start * step..end].to_vec())
                } else {
                    anyhow::bail!("Mode is an integer, but the node is not a baker");
                }
            }
            Some(Mode::Every(_)) => {
                anyhow::bail!("Integer mode cannot be 0.");
            }
            None => (false, accounts),
        };

        // Get the initial nonce.
        let nonce = args
            .client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;
        anyhow::ensure!(nonce.all_final, "Not all transactions are finalized.");

        let rng = StdRng::from_entropy();
        Ok(Self {
            args,
            amount: ccd_args.amount,
            accounts,
            random,
            rng,
            count: 0,
            nonce: nonce.nonce,
        })
    }
}

impl Generate for CcdGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let next_account = if self.random {
            let n = self.rng.gen_range(0, self.accounts.len());
            self.accounts[n]
        } else {
            self.accounts[self.count % self.accounts.len()]
        };

        let expiry = TransactionTime::seconds_after(self.args.expiry);
        let tx = send::transfer(
            &*self.args.keys,
            self.args.keys.address,
            self.nonce,
            expiry,
            next_account,
            self.amount,
        );

        self.nonce.next_mut();
        self.count += 1;

        Ok(tx)
    }
}

struct MintCis2Generator {
    args:             CommonArgs,
    contract_address: ContractAddress,
    nonce:            Nonce,
    next_id:          u32,
}

#[derive(concordium_std::Serial)]
struct MintCis2NftParams {
    owner:  concordium_std::Address,
    #[concordium(size_length = 1)]
    tokens: collections::BTreeSet<TokenId>,
}

impl MintCis2Generator {
    async fn instantiate(mut args: CommonArgs) -> anyhow::Result<Self> {
        // Get the initial nonce.
        let mut nonce = args
            .client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        let info = ContractDeploymentInfo {
            module:      MINT_CIS2_MODULE,
            name:        "init_cis2_nft",
            init_energy: Energy::from(2397),
        };
        let contract_address = args
            .deploy_and_init_contract(info, &mut nonce.nonce)
            .await
            .context("Could not deploy/init the contract.")?;

        Ok(Self {
            args,
            contract_address,
            nonce: nonce.nonce,
            next_id: 0,
        })
    }
}

impl Generate for MintCis2Generator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let params = MintCis2NftParams {
            owner:  Address::Account(self.args.keys.address),
            tokens: [TokenId::new_u32(self.next_id)].into(),
        };

        let message = OwnedParameter::from_serial(&params)?;
        let receive_name = OwnedReceiveName::new("cis2_nft.mint".into())?;
        let payload = UpdateContractPayload {
            amount: Amount::zero(),
            address: self.contract_address,
            receive_name,
            message,
        };

        let expiry = TransactionTime::seconds_after(self.args.expiry);
        let tx = send::update_contract(
            &*self.args.keys,
            self.args.keys.address,
            self.nonce,
            expiry,
            payload,
            // TODO: What to do when the number of accounts in the contract increases?
            Energy::from(3500),
        );
        self.nonce.next_mut();
        self.next_id += 1;

        Ok(tx)
    }
}

struct TransferCis2Generator {
    args:             CommonArgs,
    contract_address: ContractAddress,
    accounts:         Vec<AccountAddress>,
    nonce:            Nonce,
    count:            usize,
}

#[derive(concordium_std::Serial)]
struct MintCis2TokenParam {
    token_amount: TokenAmount,
    metadata_url: MetadataUrl,
}

#[derive(concordium_std::Serial)]
struct MintCis2TokenParams {
    owner:  Address,
    tokens: BTreeMap<TokenId, MintCis2TokenParam>,
}

impl TransferCis2Generator {
    async fn instantiate(
        mut args: CommonArgs,
        transfer_cis2_args: TransferCis2Args,
    ) -> anyhow::Result<Self> {
        let accounts: Vec<AccountAddress> = match transfer_cis2_args.receivers {
            None => {
                args.client
                    .get_account_list(BlockIdentifier::LastFinal)
                    .await
                    .context("Could not obtain a list of accounts.")?
                    .response
                    .try_collect()
                    .await?
            }
            Some(receivers) => serde_json::from_str(
                &std::fs::read_to_string(receivers)
                    .context("Could not read the receivers file.")?,
            )
            .context("Could not parse the receivers file.")?,
        };
        anyhow::ensure!(!accounts.is_empty(), "List of receivers must not be empty.");

        // Get the initial nonce.
        let mut nonce = args
            .client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        let info = ContractDeploymentInfo {
            module:      TRANSFER_CIS2_MODULE,
            name:        "init_cis2_multi",
            init_energy: Energy::from(2353),
        };
        let contract_address = args
            .deploy_and_init_contract(info, &mut nonce.nonce)
            .await
            .context("Could not deploy/init the contract.")?;

        // The rest of the function mints u64::MAX tokens for the sender.
        println!("Minting u64::MAX tokens for ourselves...");

        let param = MintCis2TokenParam {
            token_amount: TokenAmount::from(u64::MAX),
            metadata_url: MetadataUrl::new("https://example.com".into(), None)?,
        };
        let params = MintCis2TokenParams {
            owner:  Address::Account(args.keys.address),
            tokens: [(TokenId::new_u8(0), param)].into(),
        };

        let message =
            OwnedParameter::from_serial(&params).context("Parameters exceeded maximum size.")?;
        let receive_name = OwnedReceiveName::new("cis2_multi.mint".into())
            .context("Name was not correctly formatted.")?;
        let payload = UpdateContractPayload {
            amount: Amount::zero(),
            address: contract_address,
            receive_name,
            message,
        };

        let expiry = TransactionTime::seconds_after(args.expiry);
        let mint_tx = send::update_contract(
            &*args.keys,
            args.keys.address,
            nonce.nonce,
            expiry,
            payload,
            Energy::from(2740),
        );
        nonce.nonce.next_mut();

        let item = BlockItem::AccountTransaction(mint_tx);
        let transaction_hash = args.client.send_block_item(&item).await?;
        let (_, summary) = args.client.wait_until_finalized(&transaction_hash).await?;
        anyhow::ensure!(
            summary.is_success(),
            "Mint transaction failed (hash = {transaction_hash})."
        );
        println!(
            "Minted u64::MAX tokens (hash = {transaction_hash}, energy = {}).",
            summary.energy_cost,
        );

        Ok(Self {
            args,
            contract_address,
            accounts,
            nonce: nonce.nonce,
            count: 0,
        })
    }
}

impl Generate for TransferCis2Generator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let next_account = self.accounts[self.count % self.accounts.len()];
        let params = TransferParams::new(
            [Transfer {
                token_id: TokenId::new_u8(0),
                amount:   TokenAmount::from(1u32),
                from:     Address::Account(self.args.keys.address),
                to:       Receiver::Account(next_account),
                data:     AdditionalData::new(vec![])?,
            }]
            .to_vec(),
        )?;

        let message = OwnedParameter::from_serial(&params)?;
        let receive_name = OwnedReceiveName::new("cis2_multi.transfer".into())?;
        let payload = UpdateContractPayload {
            amount: Amount::zero(),
            address: self.contract_address,
            receive_name,
            message,
        };

        let expiry = TransactionTime::seconds_after(self.args.expiry);
        let tx = send::update_contract(
            &*self.args.keys,
            self.args.keys.address,
            self.nonce,
            expiry,
            payload,
            // TODO: What to do when the number of accounts in the contract increases?
            Energy::from(3500),
        );
        self.nonce.next_mut();
        self.count += 1;

        Ok(tx)
    }
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
        client,
        keys: Arc::new(keys),
        tps: app.tps,
        expiry: app.expiry,
    };

    match app.command {
        Command::Ccd(ccd_args) => {
            let generator = CcdGenerator::instantiate(args.clone(), ccd_args).await?;
            generate_transactions(args, generator).await
        }
        Command::MintNfts => {
            let generator = MintCis2Generator::instantiate(args.clone()).await?;
            generate_transactions(args, generator).await
        }
        Command::TransferCis2(transfer_cis2_args) => {
            let generator =
                TransferCis2Generator::instantiate(args.clone(), transfer_cis2_args).await?;
            generate_transactions(args, generator).await
        }
    }
}

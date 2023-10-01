use anyhow::Context;
use clap::Args;
use concordium_rust_sdk::{
    base::cis4_types::CredentialInfo,
    cis2::{
        AdditionalData, Cis2Contract, Cis2TransactionMetadata, Receiver, TokenAmount, TokenId,
        Transfer,
    },
    cis4::{Cis4Contract, Cis4TransactionMetadata},
    common::{
        types::{Amount, KeyPair, TransactionTime},
        Deserial,
    },
    contract_client::{ContractTransactionMetadata, MetadataUrl, SchemaRef},
    id::types::AccountAddress,
    smart_contracts::{common as concordium_std, common::Timestamp},
    types::{
        smart_contracts::{OwnedContractName, OwnedParameter, WasmModule},
        transactions::{
            send, send::GivenEnergy, AccountTransaction, BlockItem, EncodedPayload,
            InitContractPayload,
        },
        Address, ContractAddress, Energy, NodeDetails, Nonce, WalletAccount,
    },
    v2::{self, BlockIdentifier},
    web3id::CredentialHolderId,
};
use futures::TryStreamExt;
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::{collections, collections::BTreeMap, io::Cursor, path::PathBuf, str::FromStr};

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

// All contracts were taken from
// https://github.com/Concordium/concordium-rust-smart-contracts/tree/fcc668d87207aaf07b43f5a3b02b6d0a634368d0/examples
// They were built by running `cargo concordium build` (cargo-concordium version
// 3.0.0, Rust version 1.70.0) in their respective directories.
const MINT_CIS2_MODULE: &[u8] = include_bytes!("../resources/cis2_nft.wasm.v1");
const TRANSFER_CIS2_MODULE: &[u8] = include_bytes!("../resources/cis2_multi.wasm.v1");
const WCCD_MODULE: &[u8] = include_bytes!("../resources/cis2_wccd.wasm.v1");
const REGISTER_CREDENTIALS_MODULE: &[u8] =
    include_bytes!("../resources/credential_registry.wasm.v1");

/// Info needed to deploy and initialize a contract.
struct ContractDeploymentInfo {
    /// The module to deploy.
    module:      &'static [u8],
    /// The name of the init function, e.g. "init_cis2_nft".
    name:        &'static str,
    /// The energy needed to initialize the contract.
    init_energy: Energy,
}

impl ContractDeploymentInfo {
    /// Deploys and initializes a contract based on a [`ContractDeploymentInfo`]
    /// and an [`OwnedParameter`] to the init function. Also uses and increments
    /// a supplied nonce.
    async fn deploy_and_init_contract(
        &self,
        client: &mut v2::Client,
        args: &CommonArgs,
        param: OwnedParameter,
        nonce: &mut Nonce,
    ) -> anyhow::Result<ContractAddress> {
        println!("Deploying and initializing contract...");

        // Deploy module.
        let expiry: TransactionTime = TransactionTime::seconds_after(args.expiry);
        let module = WasmModule::deserial(&mut Cursor::new(self.module))?;
        let mod_ref = module.get_module_ref();
        let deploy_tx = send::deploy_module(&args.keys, args.keys.address, *nonce, expiry, module);
        nonce.next_mut();

        let item = BlockItem::AccountTransaction(deploy_tx);
        client.send_block_item(&item).await?;

        // We don't need to wait for deployment finalization, so we can send the init
        // transaction.
        let payload = InitContractPayload {
            amount: Amount::zero(),
            mod_ref,
            init_name: OwnedContractName::new(self.name.into())?,
            param,
        };
        let init_tx = send::init_contract(
            &args.keys,
            args.keys.address,
            *nonce,
            expiry,
            payload,
            self.init_energy,
        );
        nonce.next_mut();

        let item = BlockItem::AccountTransaction(init_tx);
        let transaction_hash = client.send_block_item(&item).await?;
        // Wait until contract is initialized.
        let (_, summary) = client.wait_until_finalized(&transaction_hash).await?;
        anyhow::ensure!(
            summary.is_success(),
            "Contract init transaction failed (hash = {transaction_hash})."
        );
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

/// Arguments used by all transaction generators.
pub struct CommonArgs {
    pub keys:   WalletAccount,
    pub expiry: u32,
}

/// A transaction generator.
pub trait Generate {
    /// Generate a transaction. Will be called in a loop.
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>>;
}

pub async fn generate_transactions(
    mut client: v2::Client,
    mut generator: impl Generate + Send + 'static,
    tps: u16,
) -> anyhow::Result<()> {
    // Create a channel between the task signing and the task sending transactions.
    let (sender, mut rx) = tokio::sync::mpsc::channel(100);

    // A task that will generate and sign transactions. Spawn it to run in the
    // background.
    tokio::spawn(async move {
        loop {
            let tx = generator.generate();
            sender.send(tx).await.expect("Error in receiver");
        }
    });

    let mut interval = tokio::time::interval(tokio::time::Duration::from_micros(
        1_000_000 / u64::from(tps),
    ));
    loop {
        interval.tick().await;
        if let Some(tx) = rx.recv().await.transpose()? {
            let nonce = tx.header.nonce;
            let energy = tx.header.energy_amount;
            let item = BlockItem::AccountTransaction(tx);
            let transaction_hash = client.send_block_item(&item).await?;
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

/// A generator that makes CCD transactions for a list of accounts.
pub struct CcdGenerator {
    args:     CommonArgs,
    amount:   Amount,
    accounts: Vec<AccountAddress>,
    random:   bool,
    rng:      StdRng,
    count:    usize,
    nonce:    Nonce,
}

impl CcdGenerator {
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        ccd_args: CcdArgs,
    ) -> anyhow::Result<Self> {
        // Get the list of receivers.
        let accounts: Vec<AccountAddress> = match ccd_args.receivers {
            None => {
                client
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

        // Filter accounts based on mode.
        let (random, accounts) = match ccd_args.mode {
            Some(Mode::Random) => (true, accounts),
            Some(Mode::Every(n)) if n > 0 => {
                let ni = client.get_node_info().await?;
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
        let nonce = client
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
            &self.args.keys,
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

/// A generator that makes transactions that mints CIS-2 NFT tokens for the
/// sender.
pub struct MintCis2Generator {
    client:  Cis2Contract,
    args:    CommonArgs,
    nonce:   Nonce,
    next_id: u32,
}

#[derive(concordium_std::Serial)]
struct MintCis2NftParams {
    owner:  concordium_std::Address,
    #[concordium(size_length = 1)]
    tokens: collections::BTreeSet<TokenId>,
}

impl MintCis2Generator {
    pub async fn instantiate(mut client: v2::Client, args: CommonArgs) -> anyhow::Result<Self> {
        // Get the initial nonce.
        let mut nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        // Deploy and initialize the contract.
        let info = ContractDeploymentInfo {
            module:      MINT_CIS2_MODULE,
            name:        "init_cis2_nft",
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client.
            init_energy: Energy::from(2397),
        };
        let contract_address = info
            .deploy_and_init_contract(
                &mut client,
                &args,
                OwnedParameter::empty(),
                &mut nonce.nonce,
            )
            .await
            .context("Could not deploy/init the contract.")?;

        let client = Cis2Contract::create(client, contract_address).await?;
        Ok(Self {
            client,
            args,
            nonce: nonce.nonce,
            next_id: 0,
        })
    }
}

impl Generate for MintCis2Generator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        // We mint a single token for ourselves.
        let params = MintCis2NftParams {
            owner:  Address::Account(self.args.keys.address),
            tokens: [TokenId::new_u32(self.next_id)].into(),
        };

        let metadata = ContractTransactionMetadata {
            sender_address: self.args.keys.address,
            nonce:          self.nonce,
            expiry:         TransactionTime::seconds_after(self.args.expiry),
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client and then adding extra to account for variance.
            energy:         GivenEnergy::Absolute(Energy::from(3500)),
            amount:         Amount::zero(),
        };
        let tx = self.client.make_update::<_, anyhow::Error>(
            &self.args.keys,
            &metadata,
            "mint",
            &params,
        )?;
        self.nonce.next_mut();
        self.next_id += 1;

        Ok(tx)
    }
}

/// A generator that makes transactions that transfer CIS-2 tokens to a list of
/// accounts.
pub struct TransferCis2Generator {
    client:   Cis2Contract,
    args:     CommonArgs,
    accounts: Vec<AccountAddress>,
    nonce:    Nonce,
    count:    usize,
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
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        transfer_cis2_args: TransferCis2Args,
    ) -> anyhow::Result<Self> {
        // Get the list of receivers.
        let accounts: Vec<AccountAddress> = match transfer_cis2_args.receivers {
            None => {
                client
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
        let mut nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        // Deploy and initialize the contract.
        let info = ContractDeploymentInfo {
            module:      TRANSFER_CIS2_MODULE,
            name:        "init_cis2_multi",
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client.
            init_energy: Energy::from(2353),
        };
        let contract_address = info
            .deploy_and_init_contract(
                &mut client,
                &args,
                OwnedParameter::empty(),
                &mut nonce.nonce,
            )
            .await
            .context("Could not deploy/init the contract.")?;

        let mut client = Cis2Contract::create(client, contract_address).await?;

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

        let metadata = ContractTransactionMetadata {
            sender_address: args.keys.address,
            nonce:          nonce.nonce,
            expiry:         TransactionTime::seconds_after(args.expiry),
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client.
            energy:         GivenEnergy::Absolute(Energy::from(2740)),
            amount:         Amount::zero(),
        };
        let transaction_hash = client
            .update::<_, anyhow::Error>(&args.keys, &metadata, "mint", &params)
            .await?;
        nonce.nonce.next_mut();

        let (_, summary) = client
            .client
            .wait_until_finalized(&transaction_hash)
            .await?;
        anyhow::ensure!(
            summary.is_success(),
            "Mint transaction failed (hash = {transaction_hash})."
        );
        println!(
            "Minted u64::MAX tokens (hash = {transaction_hash}, energy = {}).",
            summary.energy_cost,
        );

        Ok(Self {
            client,
            args,
            accounts,
            nonce: nonce.nonce,
            count: 0,
        })
    }
}

impl Generate for TransferCis2Generator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let next_account = self.accounts[self.count % self.accounts.len()];
        let transfer = Transfer {
            token_id: TokenId::new_u8(0),
            amount:   TokenAmount::from(1u32),
            from:     Address::Account(self.args.keys.address),
            to:       Receiver::Account(next_account),
            data:     AdditionalData::new(vec![])?,
        };

        let metadata = Cis2TransactionMetadata {
            sender_address: self.args.keys.address,
            nonce:          self.nonce,
            expiry:         TransactionTime::seconds_after(self.args.expiry),
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client and then adding extra to account for variance.
            energy:         GivenEnergy::Absolute(Energy::from(3500)),
            amount:         Amount::zero(),
        };
        let tx = self
            .client
            .make_transfer_single(&self.args.keys, metadata, transfer)?;
        self.nonce.next_mut();
        self.count += 1;

        Ok(tx)
    }
}

/// A generator that makes transactions that wrap, unwrap, and transfer WCCDs.
pub struct WccdGenerator {
    client: Cis2Contract,
    args:   CommonArgs,
    nonce:  Nonce,
    count:  usize,
}

#[derive(concordium_std::Serial)]
struct SetMetadataUrlParams {
    url:  String,
    hash: Option<[u8; 32]>,
}

#[derive(concordium_std::Serial)]
struct WrapParams {
    to:   Receiver,
    data: AdditionalData,
}

#[derive(concordium_std::Serial)]
struct UnwrapParams {
    amount:   TokenAmount,
    owner:    Address,
    receiver: Address,
    data:     AdditionalData,
}

impl WccdGenerator {
    pub async fn instantiate(mut client: v2::Client, args: CommonArgs) -> anyhow::Result<Self> {
        // Get the initial nonce.
        let mut nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        let info = ContractDeploymentInfo {
            module:      WCCD_MODULE,
            name:        "init_cis2_wCCD",
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client.
            init_energy: Energy::from(2596),
        };
        let params = SetMetadataUrlParams {
            url:  "https://example.com".into(),
            hash: None,
        };
        let contract_address = info
            .deploy_and_init_contract(
                &mut client,
                &args,
                OwnedParameter::from_serial(&params)?,
                &mut nonce.nonce,
            )
            .await
            .context("Could not deploy/init the contract.")?;

        // Give everyone on the network a wCCD token to increase the size of the state
        // of the contract.
        println!("Minting wCCD tokens for everyone...");
        let receivers: Vec<_> = client
            .get_account_list(BlockIdentifier::LastFinal)
            .await
            .context("Could not obtain a list of accounts.")?
            .response
            .try_collect()
            .await?;

        let mut client = Cis2Contract::create(client, contract_address).await?;

        for recv in receivers {
            let params = WrapParams {
                to:   Receiver::Account(recv),
                data: AdditionalData::new(vec![])?,
            };

            let metadata = ContractTransactionMetadata {
                sender_address: args.keys.address,
                nonce:          nonce.nonce,
                expiry:         TransactionTime::seconds_after(args.expiry),
                energy:         GivenEnergy::Absolute(Energy::from(3500)),
                amount:         Amount::from_micro_ccd(1),
            };
            let transaction_hash = client
                .update::<_, anyhow::Error>(&args.keys, &metadata, "wrap", &params)
                .await?;
            nonce.nonce.next_mut();
            println!(
                "{}: Transferred 1 wCCD to {recv} (hash = {transaction_hash}).",
                chrono::Utc::now(),
            );
        }

        Ok(Self {
            client,
            args,
            nonce: nonce.nonce,
            count: 0,
        })
    }
}

impl Generate for WccdGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let mut metadata = ContractTransactionMetadata {
            sender_address: self.args.keys.address,
            nonce:          self.nonce,
            expiry:         TransactionTime::seconds_after(self.args.expiry),
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client and then adding extra to account for variance.
            energy:         GivenEnergy::Absolute(Energy::from(3500)),
            amount:         Amount::zero(),
        };

        // We modulate between wrapping, transferring, and unwrapping. All wCCD are
        // minted for and transferred to our own account, which is fine for testing,
        // since there is no special logic for this in the contract.
        let tx = match self.count % 3 {
            // Wrap
            0 => {
                let params = WrapParams {
                    to:   Receiver::Account(self.args.keys.address),
                    data: AdditionalData::new(vec![])?,
                };
                metadata.amount = Amount::from_micro_ccd(1);

                self.client.make_update::<_, anyhow::Error>(
                    &self.args.keys,
                    &metadata,
                    "wrap",
                    &params,
                )?
            }
            // Transfer
            1 => {
                let transfer = Transfer {
                    // The token id of wCCD is the empty list of bytes.
                    token_id: TokenId::new(vec![])?,
                    amount:   TokenAmount::from(1u32),
                    from:     Address::Account(self.args.keys.address),
                    to:       Receiver::Account(self.args.keys.address),
                    data:     AdditionalData::new(vec![])?,
                };

                self.client
                    .make_transfer_single(&self.args.keys, metadata, transfer)?
            }
            // Unwrap
            _ => {
                let params = UnwrapParams {
                    amount:   TokenAmount::from(1u32),
                    owner:    Address::Account(self.args.keys.address),
                    receiver: Address::Account(self.args.keys.address),
                    data:     AdditionalData::new(vec![])?,
                };

                self.client.make_update::<_, anyhow::Error>(
                    &self.args.keys,
                    &metadata,
                    "unwrap",
                    &params,
                )?
            }
        };
        self.nonce.next_mut();
        self.count += 1;

        Ok(tx)
    }
}

/// A generator that makes transactions that register dummy Web3 ID credentials.
pub struct RegisterCredentialsGenerator {
    client: Cis4Contract,
    args:   CommonArgs,
    nonce:  Nonce,
    rng:    StdRng,
}

#[derive(concordium_std::Serial)]
struct CredentialType {
    #[concordium(size_length = 1)]
    credential_type: String,
}

#[derive(concordium_std::Serial)]
struct RegisterCredentialsInitParams {
    issuer_metadata: MetadataUrl,
    credential_type: CredentialType,
    schema:          SchemaRef,
    issuer_account:  Option<AccountAddress>,
    issuer_key:      CredentialHolderId,
    #[concordium(size_length = 1)]
    revocation_keys: Vec<CredentialHolderId>,
}

#[derive(concordium_std::Serial)]
struct RegisterCredentialParams {
    credential_info: CredentialInfo,
    #[concordium(size_length = 2)]
    auxiliary_data:  Vec<u8>,
}

impl RegisterCredentialsGenerator {
    pub async fn instantiate(mut client: v2::Client, args: CommonArgs) -> anyhow::Result<Self> {
        // Get the initial nonce.
        let mut nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;

        let mut rng = StdRng::from_entropy();
        let issuer_public_key = KeyPair::generate(&mut rng).public;

        let info = ContractDeploymentInfo {
            module:      REGISTER_CREDENTIALS_MODULE,
            name:        "init_credential_registry",
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client.
            init_energy: Energy::from(2970),
        };
        // The parameters don't matter, since we can still issue, so they are all dummy
        // values.
        let params = RegisterCredentialsInitParams {
            issuer_metadata: MetadataUrl::new("https://example.com".into(), None)?,
            credential_type: CredentialType {
                credential_type: "TestCredential".into(),
            },
            schema:          SchemaRef {
                schema_ref: MetadataUrl::new("https://example.com".into(), None)?,
            },
            issuer_account:  None,
            issuer_key:      CredentialHolderId::new(issuer_public_key),
            revocation_keys: vec![],
        };

        let contract_address = info
            .deploy_and_init_contract(
                &mut client,
                &args,
                OwnedParameter::from_serial(&params)?,
                &mut nonce.nonce,
            )
            .await
            .context("Could not deploy/init the contract.")?;

        let client = Cis4Contract::create(client, contract_address).await?;
        Ok(Self {
            client,
            args,
            nonce: nonce.nonce,
            rng,
        })
    }
}

impl Generate for RegisterCredentialsGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        // Create 32 byte holder id.
        let public_key = KeyPair::generate(&mut self.rng).public;

        let cred_info = CredentialInfo {
            holder_id:        CredentialHolderId::new(public_key),
            holder_revocable: false,
            valid_from:       Timestamp::from_timestamp_millis(0),
            valid_until:      None,
            metadata_url:     MetadataUrl::new("https://example.com".into(), None)?,
        };

        let metadata = Cis4TransactionMetadata {
            sender_address: self.args.keys.address,
            nonce:          self.nonce,
            expiry:         TransactionTime::seconds_after(self.args.expiry),
            // Determined by running the transaction and inspecting the energy cost with
            // concordium-client and then adding extra to account for variance.
            energy:         GivenEnergy::Absolute(Energy::from(5000)),
            amount:         Amount::zero(),
        };
        let tx =
            self.client
                .make_register_credential(&self.args.keys, &metadata, &cred_info, &[])?;
        self.nonce.next_mut();

        Ok(tx)
    }
}

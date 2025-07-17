use crate::generator::{CommonArgs, Generate};
use anyhow::{ensure, Context};
use clap::{Args, Subcommand};
use concordium_rust_sdk::common::types::{AccountAddress, Amount, TransactionTime};
use concordium_rust_sdk::protocol_level_tokens::{operations, CborHolderAccount, CborTokenHolder, CoinInfo, ConversionRule, MetadataUrl, RawCbor, TokenAmount, TokenId, TokenInfo, TokenModuleInitializationParameters, TokenModuleRef};
use concordium_rust_sdk::types::transactions::{
    send, AccountTransaction, BlockItem, EncodedPayload,
};
use concordium_rust_sdk::types::{
    CreatePlt, Nonce, UpdateHeader, UpdateInstruction, UpdateInstructionSignature, UpdatePayload,
    UpdateSequenceNumber,
};
use concordium_rust_sdk::v2::BlockIdentifier;
use concordium_rust_sdk::{common, v2};
use futures::TryStreamExt;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use std::path::PathBuf;
use std::str::FromStr;
use concordium_rust_sdk::common::cbor;

#[derive(Debug, Args)]
pub struct PltOperationArgs {
    #[arg(long = "targets", help = "path to file containing receivers/targets")]
    targets: Option<PathBuf>,
    #[clap(long = "token", help = "PLT token to use")]
    token: TokenId,
    #[clap(
        long = "amount",
        help = "token amount to use in each PLT operation (transfer/mint/burn)",
        default_value = "0.00001"
    )]
    amount: Decimal,
    #[command(subcommand)]
    plt_operation: PltOperation,
}

#[derive(Debug, Subcommand)]
enum PltOperation {
    Transfer,
    MintBurn,
    AddRemoveAllowDeny,
}

/// A generator that creates PLT operations
pub struct PltOperationGenerator {
    args: CommonArgs,
    plt_operation: PltOperation,
    amount: TokenAmount,
    accounts: Vec<AccountAddress>,
    rng: StdRng,
    nonce: Nonce,
    token_info: TokenInfo,
}

impl PltOperationGenerator {
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        plt_args: PltOperationArgs,
    ) -> anyhow::Result<Self> {
        let accounts: Vec<AccountAddress> = match plt_args.targets {
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
        println!("found {} accounts", accounts.len());

        let nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;
        anyhow::ensure!(nonce.all_final, "not all transactions are finalized.");
        println!("current account nonce: {}", nonce.nonce);

        let token_info = client
            .get_token_info(plt_args.token.clone(), BlockIdentifier::LastFinal)
            .await
            .context("fetch token info for token id")?
            .response;
        let amount = TokenAmount::try_from_rust_decimal(
            plt_args.amount,
            token_info.token_state.decimals,
            ConversionRule::Exact,
        )
        .context("convert token amount")?;

        let rng = StdRng::from_rng(rand::thread_rng())?;
        Ok(Self {
            args,
            amount,
            accounts,
            rng,
            token_info,
            nonce: nonce.nonce,
            plt_operation: plt_args.plt_operation,
        })
    }

    fn random_account(&mut self) -> AccountAddress {
        self.accounts
            .choose(&mut self.rng)
            .expect("accounts never initialized empty")
            .clone()
    }
}

impl Generate for PltOperationGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let expiry = TransactionTime::seconds_after(self.args.expiry);

        let operation = match self.plt_operation {
            PltOperation::Transfer => {
                operations::transfer_tokens(self.random_account(), self.amount)
            }
            PltOperation::MintBurn => {
                let mint: bool = self.rng.gen();
                if mint {
                    operations::mint_tokens(self.amount)
                } else {
                    operations::burn_tokens(self.amount)
                }
            }
            PltOperation::AddRemoveAllowDeny => match rand::thread_rng().gen_range(0..4) {
                0 => operations::add_token_allow_list(self.random_account()),
                1 => operations::remove_token_allow_list(self.random_account()),
                2 => operations::add_token_deny_list(self.random_account()),
                3 => operations::remove_token_deny_list(self.random_account()),
                _ => unreachable!(),
            },
        };

        let txn = send::token_update_operations(
            &self.args.keys,
            self.args.keys.address,
            self.nonce,
            expiry,
            self.token_info.token_id.clone(),
            [operation].into_iter().collect(),
        )?;

        self.nonce.next_mut();

        Ok(txn)
    }
}

#[derive(Debug, Args)]
pub struct CreatePltArgs {
    #[clap(
        long = "amount",
        help = "token amount to initialize token with",
        default_value = "1000000000000"
    )]
    amount: Decimal,
    #[clap(long = "update-key", help = "path to file containing update key")]
    update_keys:  Vec<PathBuf>,
    #[clap(long = "count", help = "number of PLTs to create")]
    count: Option<usize>,
}

/// A generator that creates PLTs
pub struct CreatePltGenerator {
    args: CommonArgs,
    initial_supply: TokenAmount,
    rng: StdRng,
    created: usize,
    count: Option<usize>,
    update_sequence: UpdateSequenceNumber,
    governance_account: AccountAddress,
}

impl CreatePltGenerator {
    const DECIMALS: u8 = 6;

    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        plt_args: CreatePltArgs,
    ) -> anyhow::Result<Self> {
        let update_sequence = client
            .get_next_update_sequence_numbers(BlockIdentifier::LastFinal)
            .await
            .context("could not fetch update sequence numbers")?
            .response
            .protocol_level_tokens;
        println!("current PLT update sequence: {}", update_sequence);

        let amount = TokenAmount::try_from_rust_decimal(
            plt_args.amount,
            Self::DECIMALS,
            ConversionRule::Exact,
        )
        .context("convert token amount")?;

        let rng = StdRng::from_rng(rand::thread_rng())?;
        Ok(Self {
            governance_account: args.keys.address,
            args,
            initial_supply: amount,
            rng,
            created: 0,
            count: plt_args.count,
            update_sequence,
        })
    }
}

impl Generate for CreatePltGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        unreachable!()
    }

    fn generate_block_item(&mut self) -> anyhow::Result<BlockItem<EncodedPayload>> {
        if let Some(count) = self.count {
            ensure!(self.created < count, "already created {} PLTs", self.created);
        }

        let timeout = TransactionTime::seconds_after(self.args.expiry);

        let token_id: String = (0..16).map(|_| (self.rng.gen_range('A'..'Z'))).collect();

        let initialization_parameters = TokenModuleInitializationParameters {
            name: token_id.clone(),
            metadata: MetadataUrl {
                url: "http://test".to_string(),
                checksum_sha_256: None,
                additional: Default::default(),
            },
            governance_account: CborTokenHolder::Account(CborHolderAccount {
                coin_info: Some(CoinInfo::CCD),
                address: self.governance_account,
            }),
            allow_list: Some(self.rng.gen()),
            deny_list: Some(self.rng.gen()),
            initial_supply: Some(self.initial_supply),
            mintable: Some(self.rng.gen()),
            burnable: Some(self.rng.gen()),
        };

        let create_plt = CreatePlt {
            token_id: TokenId::from_str(&token_id).context("create token id")?,
            token_module: TokenModuleRef::from_str(
                "5c5c2645db84a7026d78f2501740f60a8ccb8fae5c166dc2428077fd9a699a4a",
            )?,
            decimals: Self::DECIMALS,
            initialization_parameters: RawCbor::from(cbor::cbor_encode(&initialization_parameters)?),
        };

        let payload = UpdatePayload::CreatePlt(create_plt);

        let update_instr = UpdateInstruction {
            header: UpdateHeader {
                seq_number: self.update_sequence,
                effective_time: 0.into(),
                timeout,
                payload_size: u32::try_from(common::to_bytes(&payload).len())?.into(),
            },
            payload,
            signatures: UpdateInstructionSignature { signatures: Default::default() },// todo ar
        };

        self.update_sequence.next_mut();
        self.created += 1;

        Ok(BlockItem::UpdateInstruction(update_instr))
    }
}

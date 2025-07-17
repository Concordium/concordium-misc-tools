use crate::generator::{CommonArgs, Generate};
use anyhow::Context;
use clap::{Args, Subcommand};
use concordium_rust_sdk::common::types::{AccountAddress, Amount, TransactionTime};
use concordium_rust_sdk::protocol_level_tokens::{operations, ConversionRule, TokenAmount, TokenId, TokenInfo};
use concordium_rust_sdk::types::transactions::{send, AccountTransaction, EncodedPayload};
use concordium_rust_sdk::types::Nonce;
use concordium_rust_sdk::v2;
use concordium_rust_sdk::v2::BlockIdentifier;
use futures::TryStreamExt;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use std::path::PathBuf;

#[derive(Debug, Args)]
pub struct PltArgs {
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
pub struct PltGenerator {
    args: CommonArgs,
    plt_operation: PltOperation,
    amount: TokenAmount,
    accounts: Vec<AccountAddress>,
    rng: StdRng,
    count: usize,
    nonce: Nonce,
    token_info: TokenInfo,
}

impl PltGenerator {
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        plt_args: PltArgs,
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

        let nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;
        anyhow::ensure!(nonce.all_final, "not all transactions are finalized.");

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
            count: 0,
            nonce: nonce.nonce,
            plt_operation: plt_args.plt_operation,
        })
    }
    
    fn random_account(&mut self) -> AccountAddress {
        self
            .accounts
            .choose(&mut self.rng)
            .expect("accounts never initialized empty")
            .clone()
    }
}

impl Generate for PltGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let expiry = TransactionTime::seconds_after(self.args.expiry);
        
        let operation= match self.plt_operation {
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
            PltOperation::AddRemoveAllowDeny => {
                 match rand::thread_rng().gen_range(0..4) {
                     0 => {
                         operations::add_token_allow_list(self.random_account())     
                     }
                     1 => {
                         operations::remove_token_allow_list(self.random_account())
                     }
                     2 => {
                         operations::add_token_deny_list(self.random_account())
                     }
                     3 => {
                         operations::remove_token_deny_list(self.random_account())
                     }
                     _ => unreachable!()
                 }
            }
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
        self.count += 1;

        Ok(txn)
    }
}

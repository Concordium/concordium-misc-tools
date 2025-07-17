use std::path::PathBuf;
use anyhow::Context;
use clap::Args;
use concordium_rust_sdk::common::types::{AccountAddress, Amount, TransactionTime};
use concordium_rust_sdk::protocol_level_tokens::{TokenAmount, TokenId};
use concordium_rust_sdk::types::Nonce;
use concordium_rust_sdk::types::transactions::{send, AccountTransaction, EncodedPayload};
use concordium_rust_sdk::v2;
use concordium_rust_sdk::v2::BlockIdentifier;
use futures::TryStreamExt;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rust_decimal::Decimal;
use crate::generator::{CommonArgs, Generate};

#[derive(Debug, Args)]
pub struct PltArgs {
    #[arg(long = "receivers", help = "Path to file containing receivers.")]
    receivers: Option<PathBuf>,
    #[clap(
        long = "token",
        help = "PLT token to use",
    )]
    token:    TokenId,
    
    #[clap(
        long = "amount",
        help = "token amount to send in each transaction",
        default_value = "0.0"
    )]
    amount:    Decimal,
    
}


/// A generator that makes CCD transactions for a list of accounts.
pub struct PltGenerator {
    args:     CommonArgs,
    amount:   TokenAmount,
    accounts: Vec<AccountAddress>,
    random:   bool,
    rng:      StdRng,
    count:    usize,
    nonce:    Nonce,
}

impl PltGenerator {
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        ccd_args: PltArgs,
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
        let (random, accounts) = (false, accounts);

        // Get the initial nonce.
        let nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;
        anyhow::ensure!(nonce.all_final, "Not all transactions are finalized.");

        let rng = StdRng::from_rng(rand::thread_rng())?;
        Ok(Self {
            args,
            amount: todo!(),
            accounts,
            random,
            rng,
            count: 0,
            nonce: nonce.nonce,
        })
    }
}

impl Generate for PltGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        // let next_account = if self.random {
        //     let n = self.rng.gen_range(0..self.accounts.len());
        //     self.accounts[n]
        // } else {
        //     self.accounts[self.count % self.accounts.len()]
        // };
        // 
        // let expiry = TransactionTime::seconds_after(self.args.expiry);
        // let tx = send::transfer(
        //     &self.args.keys,
        //     self.args.keys.address,
        //     self.nonce,
        //     expiry,
        //     next_account,
        //     self.amount,
        // );
        // 
        // self.nonce.next_mut();
        // self.count += 1;
        // 
        // Ok(tx)
        todo!()
    }
}
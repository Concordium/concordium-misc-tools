use crate::generator::{CommonArgs, Generate};
use anyhow::{ensure, Context};
use clap::Args;
use concordium_rust_sdk::{
    common::{
        cbor,
        types::{AccountAddress, TransactionTime},
    },
    protocol_level_tokens::{
        operations, CborHolderAccount, CborTokenHolder, CoinInfo, ConversionRule, MetadataUrl,
        RawCbor, TokenAmount, TokenId, TokenInfo, TokenModuleInitializationParameters,
        TokenModuleRef,
    },
    types::{
        transactions::{send, AccountTransaction, BlockItem, EncodedPayload},
        update, CreatePlt, Nonce, UpdateKeyPair, UpdateKeysIndex, UpdatePayload,
        UpdateSequenceNumber,
    },
    v2,
    v2::BlockIdentifier,
};
use futures::{StreamExt, TryStreamExt};
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use rust_decimal::Decimal;
use std::{collections::HashSet, path::PathBuf, str::FromStr};

#[derive(Debug, Args)]
pub struct PltOperationArgs {
    #[arg(long = "targets", help = "path to file containing receivers/targets")]
    targets:           Option<PathBuf>,
    #[clap(
        long = "amount",
        help = "token amount to use in each PLT operation (transfer/mint/burn)",
        default_value = "0.00001"
    )]
    amount:            Decimal,
    #[clap(flatten)]
    operation_weights: OperationWeights,
}

/// Weights of operations when picking operations at random
#[derive(Debug, Args)]
pub struct OperationWeights {
    #[clap(
        long = "transfer-weight",
        help = "weight of transfers when picking operations at random",
        default_value = "10.0"
    )]
    transfer_weight:         f64,
    #[clap(
        long = "mint-burn-weight",
        help = "weight of mint+burn when picking operations at random",
        default_value = "1.0"
    )]
    mint_burn_weight:        f64,
    #[clap(
        long = "remove-add-allow-weight",
        help = "weight of remove+add from allow list when picking operations at random",
        default_value = "1.0"
    )]
    remove_add_allow_weight: f64,
    #[clap(
        long = "add-remove-deny-weight",
        help = "weight of add+remove from deny list when picking operations at random",
        default_value = "1.0"
    )]
    add_remove_deny_weight:  f64,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
enum PltOperation {
    Transfer,
    MintBurn,
    RemoveAddAllow,
    AddRemoveDeny,
}

/// A generator that creates PLT operations
pub struct PltOperationGenerator {
    args:                 CommonArgs,
    amount:               Decimal,
    /// Accounts to use as receivers/targets
    accounts:             Vec<AccountAddress>,
    rng:                  StdRng,
    nonce:                Nonce,
    /// Tokens to use
    tokens:               Vec<TokenInfo>,
    /// Tokens for which sender is on allow list
    sender_on_allow_list: HashSet<TokenId>,
    operation_weights:    OperationWeights,
}

impl PltOperationGenerator {
    pub async fn instantiate(
        mut client: v2::Client,
        args: CommonArgs,
        plt_args: PltOperationArgs,
    ) -> anyhow::Result<Self> {
        // find available accounts to use as receivers/targets
        let accounts: Vec<AccountAddress> = match plt_args.targets {
            None => {
                client
                    .get_account_list(BlockIdentifier::LastFinal)
                    .await
                    .context("Could not obtain a list of accounts.")?
                    .response
                    .try_filter(|&account| async move { account != args.keys.address })
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
        println!(
            "found {} accounts to use as receiver/target",
            accounts.len()
        );

        let nonce = client
            .get_next_account_sequence_number(&args.keys.address)
            .await?;
        anyhow::ensure!(nonce.all_final, "not all transactions are finalized.");
        println!("current sender account nonce: {}", nonce.nonce);

        // find tokens suiteable for testing the given PLT operations
        let client_clone = client.clone();
        let all_tokens = client
            .get_token_list(BlockIdentifier::LastFinal)
            .await
            .context("get token list")?
            .response
            .map_err(anyhow::Error::from)
            .and_then(|token_id| {
                let mut client = client_clone.clone();
                async move {
                    Ok(client
                        .get_token_info(token_id, BlockIdentifier::LastFinal)
                        .await
                        .context("fetch token info for token id")?
                        .response)
                }
            })
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<anyhow::Result<Vec<_>>>()
            .context("fetch token info for all tokens")?;
        let tokens = all_tokens
            .into_iter()
            .filter_map(|token| {
                let state = token.token_state.decode_module_state().ok()?;
                matches!(state.governance_account, CborTokenHolder::Account(account) if account.address == args.keys.address).then_some(token)

            })
            .collect::<Vec<_>>();

        println!("found {} tokens that are suited for testing", tokens.len(),);

        anyhow::ensure!(!tokens.is_empty(), "available tokens must not be empty");

        let rng = StdRng::from_rng(rand::thread_rng())?;
        Ok(Self {
            args,
            amount: plt_args.amount,
            accounts,
            rng,
            tokens,
            nonce: nonce.nonce,
            sender_on_allow_list: Default::default(),
            operation_weights: plt_args.operation_weights,
        })
    }

    fn random_account(&mut self) -> AccountAddress {
        *self
            .accounts
            .choose(&mut self.rng)
            .expect("accounts never initialized empty")
    }

    fn random_token_for_operation(
        &mut self,
        plt_operation: PltOperation,
    ) -> anyhow::Result<TokenInfo> {
        loop {
            let token = self
                .tokens
                .choose(&mut self.rng)
                .expect("tokens never initialized empty");

            let module_state = token.token_state.decode_module_state()?;
            let usable = match plt_operation {
                PltOperation::Transfer => true,
                PltOperation::MintBurn => {
                    module_state.mintable.unwrap_or_default()
                        && module_state.burnable.unwrap_or_default()
                }
                PltOperation::RemoveAddAllow => module_state.allow_list.unwrap_or_default(),
                PltOperation::AddRemoveDeny => module_state.deny_list.unwrap_or_default(),
            };

            if usable {
                return Ok(token.clone());
            }
        }
    }

    fn random_operation(&mut self) -> PltOperation {
        let weights = [
            (
                PltOperation::Transfer,
                self.operation_weights.transfer_weight,
            ),
            (
                PltOperation::MintBurn,
                self.operation_weights.mint_burn_weight,
            ),
            (
                PltOperation::RemoveAddAllow,
                self.operation_weights.remove_add_allow_weight,
            ),
            (
                PltOperation::AddRemoveDeny,
                self.operation_weights.add_remove_deny_weight,
            ),
        ];
        let sum = weights.iter().map(|op| op.1).sum::<f64>();
        let mut acc = 0.0;
        let rand_val = self.rng.gen_range(0.0..sum);
        for (op, weight) in weights {
            acc += weight;
            if rand_val < acc {
                return op;
            }
        }
        unreachable!()
    }
}

impl Generate for PltOperationGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> {
        let expiry = TransactionTime::seconds_after(self.args.expiry);

        let plt_operation = self.random_operation();
        let token_info = self.random_token_for_operation(plt_operation)?;

        let amount = TokenAmount::try_from_rust_decimal(
            self.amount,
            token_info.token_state.decimals,
            ConversionRule::Exact,
        )
        .context("convert token amount")?;

        let operations = match plt_operation {
            PltOperation::Transfer => {
                let mut operations =
                    vec![operations::transfer_tokens(self.random_account(), amount)];
                let module_state = token_info.token_state.decode_module_state()?;
                if module_state.allow_list.unwrap_or_default()
                    && self
                        .sender_on_allow_list
                        .insert(token_info.token_id.clone())
                {
                    operations.insert(0, operations::add_token_allow_list(self.args.keys.address));
                }

                operations
            }
            PltOperation::MintBurn => {
                vec![
                    operations::mint_tokens(amount),
                    operations::burn_tokens(amount),
                ]
            }
            PltOperation::RemoveAddAllow => {
                let target = self.random_account();
                vec![
                    operations::remove_token_allow_list(target),
                    operations::add_token_allow_list(target),
                ]
            }
            PltOperation::AddRemoveDeny => {
                let target = self.random_account();
                vec![
                    operations::add_token_deny_list(target),
                    operations::remove_token_deny_list(target),
                ]
            }
        };

        let txn = send::token_update_operations(
            &self.args.keys,
            self.args.keys.address,
            self.nonce,
            expiry,
            token_info.token_id.clone(),
            operations.into_iter().collect(),
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
    amount:      Decimal,
    #[clap(long = "update-key", help = "path to file containing update key")]
    update_keys: Vec<PathBuf>,
    #[clap(long = "count", help = "number of PLTs to create")]
    count:       Option<usize>,
}

/// A generator that creates PLTs
pub struct CreatePltGenerator {
    args:               CommonArgs,
    initial_supply:     TokenAmount,
    rng:                StdRng,
    /// Number of PLTs created so far
    created:            usize,
    /// Number of PLTs to create
    count:              Option<usize>,
    update_sequence:    UpdateSequenceNumber,
    governance_account: AccountAddress,
    update_keys:        Vec<(UpdateKeysIndex, UpdateKeyPair)>,
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

        let update_keys = plt_args
            .update_keys
            .iter()
            .map(|path| {
                // extract update key index from the file name
                let key_index: u16 = path
                    .to_str()
                    .context("not utf8 path")?
                    .strip_suffix(".json")
                    .context("update key file path must end with '.json'")?
                    .rsplit_once("-")
                    .context("update key path must have format 'level2-key-x.json'")?
                    .1
                    .parse()
                    .context(
                        "update key path must have format 'level2-key-x.json' where x is a number",
                    )?;
                let file = std::fs::File::open(path).context("unable to open key file")?;
                let key_pair: UpdateKeyPair =
                    serde_json::from_reader(file).context("parse update key JSON")?;
                Ok((UpdateKeysIndex::from(key_index), key_pair))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        let rng = StdRng::from_rng(rand::thread_rng())?;
        Ok(Self {
            governance_account: args.keys.address,
            args,
            initial_supply: amount,
            rng,
            created: 0,
            count: plt_args.count,
            update_sequence,
            update_keys,
        })
    }
}

impl Generate for CreatePltGenerator {
    fn generate(&mut self) -> anyhow::Result<AccountTransaction<EncodedPayload>> { unreachable!() }

    fn generate_block_item(&mut self) -> anyhow::Result<BlockItem<EncodedPayload>> {
        if let Some(count) = self.count {
            ensure!(
                self.created < count,
                "already created {} PLTs",
                self.created
            );
        }

        let timeout = TransactionTime::seconds_after(self.args.expiry);

        let token_id: String = (0..16).map(|_| (self.rng.gen_range('A'..='Z'))).collect();

        let mint_burn: bool = self.rng.gen();
        let allow_deny: bool = self.rng.gen();

        let initialization_parameters = TokenModuleInitializationParameters {
            name:               token_id.clone(),
            metadata:           MetadataUrl {
                url:              "http://test".to_string(),
                checksum_sha_256: None,
                additional:       Default::default(),
            },
            governance_account: CborTokenHolder::Account(CborHolderAccount {
                coin_info: Some(CoinInfo::CCD),
                address:   self.governance_account,
            }),
            allow_list:         Some(allow_deny),
            deny_list:          Some(allow_deny),
            initial_supply:     Some(self.initial_supply),
            mintable:           Some(mint_burn),
            burnable:           Some(mint_burn),
        };

        let create_plt = CreatePlt {
            token_id:                  TokenId::from_str(&token_id).context("create token id")?,
            token_module:              TokenModuleRef::from_str(
                "5c5c2645db84a7026d78f2501740f60a8ccb8fae5c166dc2428077fd9a699a4a",
            )?,
            decimals:                  Self::DECIMALS,
            initialization_parameters: RawCbor::from(cbor::cbor_encode(
                &initialization_parameters,
            )?),
        };

        let payload = UpdatePayload::CreatePlt(create_plt);

        let update_instr = update::update(
            self.update_keys.as_slice(),
            self.update_sequence,
            0.into(),
            timeout,
            payload,
        );

        self.update_sequence.next_mut();
        self.created += 1;

        Ok(BlockItem::UpdateInstruction(update_instr))
    }
}

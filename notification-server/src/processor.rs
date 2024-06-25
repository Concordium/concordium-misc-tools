use concordium_rust_sdk::{
    base::contracts_common::AccountAddress,
    cis2,
    cis2::{Event, TokenId},
    types,
    types::{
        AccountTransactionEffects, Address, Address::Account,
        BlockItemSummaryDetails::AccountTransaction, ContractTraceElement,
    },
};
use futures::{Stream, StreamExt};
use num_bigint::BigInt;
use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationInformation {
    pub address:  AccountAddress,
    pub amount:   BigInt,
}

impl NotificationInformation {
    pub fn new(address: AccountAddress, amount: BigInt) -> Self {
        Self {
            address,
            amount,
        }
    }
}

fn convert<T: Into<BigInt>>(
    address: Address,
    amount: T,
    is_positive: bool,
) -> Option<NotificationInformation> {
    let mut amount: BigInt = amount.into();
    if !is_positive {
        amount = -amount;
    }

    match address {
        Account(address) => Some(NotificationInformation::new(
            address,
            amount,
        )),
        _ => None,
    }
}

fn get_cis2_events_addresses(effects: &AccountTransactionEffects) -> Vec<NotificationInformation> {
    match &effects {
        AccountTransactionEffects::AccountTransfer { to, amount } => {
            println!("Amount: {:?}", amount.micro_ccd);
            vec![NotificationInformation::new(
                *to,
                amount.micro_ccd.into(),
            )]
        }
        AccountTransactionEffects::AccountTransferWithMemo { to, amount, .. } => {
            vec![NotificationInformation::new(
                *to,
                amount.micro_ccd.into(),
            )]
        }
        AccountTransactionEffects::ContractUpdateIssued { effects } => effects
            .iter()
            .flat_map(|effect| match effect {
                ContractTraceElement::Transferred { to, amount, .. } => {
                    vec![NotificationInformation::new(
                        *to,
                        amount.micro_ccd.into(),
                    )]
                }
                ContractTraceElement::Updated { data } => data
                    .events
                    .iter()
                    .filter_map(|event| match cis2::Event::try_from(event) {
                        Ok(Event::Transfer {
                            to,
                            amount,
                            token_id,
                            ..
                        }) => convert(to, amount.0, true),
                        Ok(Event::Mint {
                            owner,
                            amount,
                            token_id,
                        }) => convert(owner, amount.0, true),
                        Ok(Event::Burn {
                            owner,
                            amount,
                            token_id,
                        }) => convert(owner, amount.0, false),
                        _ => None,
                    })
                    .collect(),
                _ => vec![],
            })
            .collect(),
        AccountTransactionEffects::TransferredWithSchedule { to, amount } => {
            vec![NotificationInformation::new(
                *to,
                amount.iter().fold(BigInt::from(0), |acc, &(_, item)| {
                    acc + BigInt::from(item.micro_ccd)
                }),
            )]
        }
        AccountTransactionEffects::TransferredWithScheduleAndMemo { to, amount, .. } => {
            vec![NotificationInformation::new(
                *to,
                amount.iter().fold(BigInt::from(0), |acc, &(_, item)| {
                    acc + BigInt::from(item.micro_ccd)
                }),
            )]
        }
        _ => vec![],
    }
}

pub async fn process(
    transactions: impl Stream<Item = Result<types::BlockItemSummary, tonic::Status>>,
) -> Vec<NotificationInformation> {
    transactions
        .filter_map(|result| async move { result.ok() })
        .flat_map(|t| {
            futures::stream::iter(match t.details {
                AccountTransaction(ref account_transaction) => {
                    get_cis2_events_addresses(&account_transaction.effects)
                }
                _ => vec![],
            })
        })
        .collect::<Vec<NotificationInformation>>()
        .await
}

#[cfg(test)]
mod tests {
    use concordium_rust_sdk::base::hashes::HashBytes;
    use concordium_rust_sdk::common::types::Amount;
    use concordium_rust_sdk::smart_contracts::common::AccountAddress;
    use concordium_rust_sdk::types::{AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary, BlockItemSummaryDetails, Energy, hashes, Memo, TransactionIndex};
    use futures::{stream};
    use num_bigint::BigInt;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use rand::{random, Rng, thread_rng};
    use sha2::Digest;
    use crate::processor::{NotificationInformation, process};

    #[derive(Clone, Debug)]
    struct ArbitraryTransactionIndex(pub TransactionIndex);

    // Implement Arbitrary for the wrapper
    impl Arbitrary for ArbitraryTransactionIndex {
        fn arbitrary(g: &mut Gen) -> Self {
            let mut rng = rand::thread_rng();
            let index: u64 = rng.gen();
            ArbitraryTransactionIndex(TransactionIndex { index })
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryEnergy(pub Energy);

    impl Arbitrary for ArbitraryEnergy {
        fn arbitrary(g: &mut Gen) -> Self {
            ArbitraryEnergy(random_energy_cost())
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryTransactionHash(pub hashes::TransactionHash);

    impl Arbitrary for ArbitraryTransactionHash {
        fn arbitrary(_g: &mut Gen) -> Self {
            ArbitraryTransactionHash(random_hash())
        }
    }

    fn random_transaction_index() -> TransactionIndex {
        TransactionIndex {
            index: random(),
        }
    }

    fn random_energy_cost() -> Energy {
        Energy {
            energy: random(),
        }
    }


    fn random_memo() -> Memo {
        let mut rng = thread_rng();
        let size: usize = rng.gen_range(0..= 256);
        let bytes: Vec<u8> = (0..size).map(|_| rng.gen()).collect();

        Memo::try_from(bytes).expect("Generated memo exceeds the maximum size")
    }

    fn random_hash() -> hashes::TransactionHash {
        let mut hasher = sha2::Sha256::new();
        hasher.update(b"hello world");
        HashBytes::new(hasher.finalize().into())
    }

    const ACCOUNT_ADDRESS_SIZE: usize = 32;

    fn random_account_address() -> [u8; ACCOUNT_ADDRESS_SIZE] {
        let mut rng = rand::thread_rng();
        let mut address = [0u8; ACCOUNT_ADDRESS_SIZE];
        rng.fill(&mut address);
        address
    }

    fn random_block_item_summary_and_notification() -> (BlockItemSummary, NotificationInformation) {
    let amount = Amount { micro_ccd: random() };
    let receiver_address = AccountAddress(random_account_address());

    // Define the details once
    let details = |effects| AccountTransactionDetails {
        cost: amount.clone(),
        effects,
        sender: receiver_address.clone(),
    };

    // Create different effects
    let effects = vec![
        AccountTransactionEffects::AccountTransfer {
            amount: amount.clone(),
            to: receiver_address.clone(),
        },
        AccountTransactionEffects::AccountTransferWithMemo {
            amount: amount.clone(),
            to: receiver_address.clone(),
            memo: random_memo(),
        },
    ];

    // Randomly select one of the effects
    let effect = effects[random::<usize>() % effects.len()].clone();

    (
        BlockItemSummary {
            index: random_transaction_index(),
            energy_cost: random_energy_cost(),
            hash: random_hash(),
            details: BlockItemSummaryDetails::AccountTransaction(details(effect.clone())),
        },
        NotificationInformation {
            address: receiver_address,
            amount: BigInt::from(amount.micro_ccd),
        },
    )
}

    #[derive(Clone, Debug)]
    struct ValidBlockItemSummaryPair(pub BlockItemSummary, pub NotificationInformation);

    impl Arbitrary for ValidBlockItemSummaryPair {
    fn arbitrary(g: &mut Gen) -> Self {
        let amount = Amount { micro_ccd: u64::arbitrary(g) };
        let receiver_address = AccountAddress(random_account_address());

        let details = |effects| AccountTransactionDetails {
            cost: amount.clone(),
            effects,
            sender: receiver_address.clone(),
        };

        let effects = vec![
            AccountTransactionEffects::AccountTransfer {
                amount: amount.clone(),
                to: receiver_address.clone(),
            },
            AccountTransactionEffects::AccountTransferWithMemo {
                amount: amount.clone(),
                to: receiver_address.clone(),
                memo: random_memo(),
            },
        ];

        let effect = effects[usize::arbitrary(g) % effects.len()].clone();

        let summary = BlockItemSummary {
            index: random_transaction_index(),
            energy_cost: random_energy_cost(),
            hash: random_hash(),
            details: BlockItemSummaryDetails::AccountTransaction(details(effect.clone())),
        };

        let notification = NotificationInformation {
            address: receiver_address,
            amount: BigInt::from(amount.micro_ccd),
        };

        ValidBlockItemSummaryPair(summary, notification)
    }
    }

    #[quickcheck]
    fn test_random_block_item_summary(summary: ValidBlockItemSummaryPair) -> bool {
        let summaries = vec![
            Ok(summary.0)
        ];
        let summary_stream = stream::iter(summaries);
        let mut runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(process(summary_stream));
        result[0] == summary.1
    }
}

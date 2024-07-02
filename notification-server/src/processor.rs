use std::fmt::Debug;

use concordium_rust_sdk::{
    base::contracts_common::AccountAddress,
    cis2,
    cis2::Event,
    types,
    types::{
        AccountTransactionEffects, Address, Address::Account,
        BlockItemSummaryDetails::AccountTransaction, ContractTraceElement,
    },
};
use futures::{Stream, StreamExt};
use num_bigint::BigInt;

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
                            ..
                        }) => convert(to, amount.0, true),
                        Ok(Event::Mint {
                            owner,
                            amount,
                            ..
                        }) => convert(owner, amount.0, true),
                        Ok(Event::Burn {
                            owner,
                            amount,
                            ..
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
    use std::fmt::Debug;
    use std::str::FromStr;
    use concordium_rust_sdk::base::contracts_common::AccountAddress;
    use concordium_rust_sdk::base::hashes::HashBytes;
    use concordium_rust_sdk::cis2::{Event, TokenAmount, TokenId};
    use concordium_rust_sdk::common::types::{Amount, Timestamp};
    use concordium_rust_sdk::id::constants::ArCurve;
    use concordium_rust_sdk::types::{AccountCreationDetails, AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary, BlockItemSummaryDetails, CredentialRegistrationID, CredentialType, Energy, hashes, Memo, TransactionIndex};
    use concordium_rust_sdk::types::Address::Account;
    use futures::stream;
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
            let index = u64::arbitrary(g);
            ArbitraryTransactionIndex(TransactionIndex { index })
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryEnergy(pub Energy);

    impl Arbitrary for ArbitraryEnergy {
        fn arbitrary(g: &mut Gen) -> Self {
            ArbitraryEnergy(
                Energy {
                    energy: u64::arbitrary(g),
                }
            )
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryTransactionHash(pub hashes::TransactionHash);

    impl Arbitrary for ArbitraryTransactionHash {
        fn arbitrary(_g: &mut Gen) -> Self {
            ArbitraryTransactionHash(fixed_hash())
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryCredentialType(pub CredentialType);
    impl Arbitrary for ArbitraryCredentialType {
    fn arbitrary(g: &mut Gen) -> Self {

        ArbitraryCredentialType(
            *g.choose(&[CredentialType::Initial, CredentialType::Normal]).unwrap()
        )
    }
}

    fn random_transaction_index() -> TransactionIndex {
        TransactionIndex {
            index: random(),
        }
    }


    fn random_memo() -> Memo {
        let mut rng = thread_rng();
        let size: usize = rng.gen_range(0..= 256);
        let bytes: Vec<u8> = (0..size).map(|_| rng.gen()).collect();

        Memo::try_from(bytes).expect("Generated memo exceeds the maximum size")
    }

    fn fixed_hash() -> hashes::TransactionHash {
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

    fn split_u64_to_random_vec(mut value: u64, max_elements: usize, g: &mut Gen) -> Vec<(Timestamp, Amount)> {
        let mut rng = thread_rng();
        let mut result = Vec::new();

        while value > 0 && result.len() < max_elements {
            // Generate a random value between 1 and the remaining value
            let part = rng.gen_range(0..=value);
            result.push((u64::arbitrary(g).into(), Amount { micro_ccd: part }));
            value -= part;
        }

        // Add the remaining value to the result to ensure the exact sum
        if value > 0 {
            result.push((u64::arbitrary(g).into(), Amount { micro_ccd: value }));
        }
        result
    }

    fn _get_random_cis2_event(g: &mut Gen, amount: Amount, address: AccountAddress) -> Event {
        let token_id = TokenId::new("wCCD".as_bytes().to_vec()).unwrap();
        let events = vec![
            Event::Transfer {
                token_id: token_id.clone(),
                amount: TokenAmount(amount.micro_ccd.into()),
                from: Account(AccountAddress(random_account_address())),
                to: Account(address),
            },
            Event::Mint {
                token_id: token_id.clone(),
                amount: TokenAmount(amount.micro_ccd.into()),
                owner: Account(address),
            },
            Event::Burn {
                token_id: token_id.clone(),
                amount: TokenAmount(amount.micro_ccd.into()),
                owner: Account(address),
            }];
        events[usize::arbitrary(g) % events.len()].clone()
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
        //let event = get_random_cis2_event(g, amount.clone(), receiver_address.clone());
        //let serialised_event = serde_json::to_vec(&event).expect("Failed to serialize Event");



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
            AccountTransactionEffects::TransferredWithSchedule {
                to: receiver_address.clone(),
                amount: split_u64_to_random_vec(amount.clone().micro_ccd,  2, g),
            },
            AccountTransactionEffects::TransferredWithScheduleAndMemo {
                to: receiver_address.clone(),
                amount: split_u64_to_random_vec(amount.clone().micro_ccd,  2, g),
                memo: random_memo(),
            },
            //AccountTransactionEffects::ContractUpdateIssued {
            //    effects: vec! [
            //        ContractTraceElement::Updated {
            //            data: InstanceUpdatedEvent {
            //                contract_version: WasmVersion::V1,
            //                address: ContractAddress::new(1, 0),
            //                instigator: receiver_address.clone().into(),
            //                amount: amount.clone(),
            //                message: OwnedParameter::empty(),
            //                receive_name: OwnedReceiveName::new("foo.bar".to_string()).unwrap(),
            //                events: vec![
            //                ]
            //            }
            //        }
            //    ]
            //}
        ];

        let effect = effects[usize::arbitrary(g) % effects.len()].clone();

        let summary = BlockItemSummary {
            index: random_transaction_index(),
            energy_cost: ArbitraryEnergy::arbitrary(g).0,
            hash: fixed_hash(),
            details: BlockItemSummaryDetails::AccountTransaction(details(effect.clone())),
        };

        let notification = NotificationInformation {
            address: receiver_address,
            amount: BigInt::from(amount.micro_ccd),
        };

        ValidBlockItemSummaryPair(summary, notification)
        }
    }

    #[derive(Clone, Debug)]
    struct InvalidBlockItemSummary(pub BlockItemSummary);

    impl Arbitrary for InvalidBlockItemSummary {
        fn arbitrary(g: &mut Gen) -> Self {
            let amount = Amount { micro_ccd: u64::arbitrary(g) };
            let receiver_address = AccountAddress(random_account_address());

            let summary = BlockItemSummary {
                index: random_transaction_index(),
                energy_cost: ArbitraryEnergy::arbitrary(g).0,
                hash: fixed_hash(),
                details: BlockItemSummaryDetails::AccountCreation(AccountCreationDetails {
                    credential_type: ArbitraryCredentialType::arbitrary(g).0,
                    address: receiver_address,
                    reg_id: CredentialRegistrationID::from_str("8a3a87f3f38a7a507d1e85dc02a92b8bcaa859f5cf56accb3c1bc7c40e1789b4933875a38dd4c0646ca3e940a02c42d8").unwrap(),
                }),
            };
            InvalidBlockItemSummary(summary)
        }
    }
        #[derive(Clone, Debug)]
    enum BlockItemSummaryWrapper {
        Valid(ValidBlockItemSummaryPair),
        Invalid(InvalidBlockItemSummary),
    }

    impl Arbitrary for BlockItemSummaryWrapper {
        fn arbitrary(g: &mut Gen) -> Self {
            if bool::arbitrary(g) {
                BlockItemSummaryWrapper::Valid(ValidBlockItemSummaryPair::arbitrary(g))
            } else {
                BlockItemSummaryWrapper::Invalid(InvalidBlockItemSummary::arbitrary(g))
            }
        }
    }

        trait TestableBlockItemSummary {
        fn get_block_item_summary(&self) -> &BlockItemSummary;
        fn get_expected_notification(&self) -> Option<NotificationInformation>;
    }

    impl TestableBlockItemSummary for BlockItemSummaryWrapper {
        fn get_block_item_summary(&self) -> &BlockItemSummary {
            match self {
                BlockItemSummaryWrapper::Valid(pair) => &pair.0,
                BlockItemSummaryWrapper::Invalid(summary) => &summary.0,
            }
        }

        fn get_expected_notification(&self) -> Option<NotificationInformation> {
            match self {
                BlockItemSummaryWrapper::Valid(pair) => Some(pair.1.clone()),
                BlockItemSummaryWrapper::Invalid(_) => None,
            }
        }
    }

    #[quickcheck]
    fn test_random_block_item_summary(summaries: Vec<BlockItemSummaryWrapper>) -> bool {
        let summary_stream = stream::iter(summaries.clone().into_iter().map(|summary| Ok(summary.get_block_item_summary().clone())).collect::<Vec<_>>());
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(process(summary_stream));
        let expected: Vec<NotificationInformation> = summaries.into_iter()
            .filter_map(|summary| summary.get_expected_notification())
            .collect();
        result == expected
    }
}

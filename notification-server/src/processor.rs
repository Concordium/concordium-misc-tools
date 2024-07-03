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
    use bls12_381::{G1Projective};
    use std::fmt::Debug;
    use std::str::FromStr;
    use concordium_rust_sdk::base::contracts_common::AccountAddress;
    use concordium_rust_sdk::base::curve_arithmetic::arkworks_instances::ArkGroup;
    use concordium_rust_sdk::base::curve_arithmetic::Curve;
    use concordium_rust_sdk::base::elgamal::Cipher;
    use concordium_rust_sdk::base::hashes::HashBytes;
    use concordium_rust_sdk::cis2::{Event, TokenAmount, TokenId};
    use concordium_rust_sdk::common::types::{Amount, Timestamp, TransactionTime};
    use concordium_rust_sdk::id::test::test_create_ars;
    use concordium_rust_sdk::types::{AccountCreationDetails, AccountTransactionDetails, AccountTransactionEffects, BlockItemSummary, BlockItemSummaryDetails, CredentialRegistrationID, CredentialType, EncryptedSelfAmountAddedEvent, Energy, ExchangeRate, hashes, Memo, RejectReason, TransactionIndex, TransactionType, UpdateDetails, UpdatePayload};
    use concordium_rust_sdk::types::Address::Account;
    use futures::stream;
    use num_bigint::BigInt;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use rand::{random, Rng, thread_rng};
    use rand::rngs::OsRng;
    use sha2::Digest;

    use crate::processor::{NotificationInformation, process};

    type SomeCurve = ArkGroup<G1Projective>;

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
        g.choose(&events).unwrap().clone()
    }

    #[derive(Clone, Debug)]
    struct EmittingBlockItemSummaryPair(pub BlockItemSummary, pub NotificationInformation);

    impl Arbitrary for EmittingBlockItemSummaryPair {
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

        let effect = g.choose(&effects).unwrap().clone();

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

        EmittingBlockItemSummaryPair(summary, notification)
        }
    }

    #[derive(Clone, Debug)]
    struct SilentBlockItemSummary(pub BlockItemSummary);
    fn get_set_vector<C: Curve>(the_set: &[u64]) -> Vec<C::Scalar> {
        the_set.iter().copied().map(C::scalar_from_u64).collect()
    }


    impl Arbitrary for SilentBlockItemSummary {
        fn arbitrary(g: &mut Gen) -> Self {
            let amount = Amount { micro_ccd: u64::arbitrary(g) };
            let receiver_address = AccountAddress(random_account_address());
            let the_set = get_set_vector::<SomeCurve>(&[1, 7, 3, 5]);
            let v = SomeCurve::scalar_from_u64(4);
            let cipher1 = Cipher(v.clone(), v.clone());

            let account_transaction_details = |effects| AccountTransactionDetails {
                cost: amount.clone(),
                effects,
                sender: receiver_address.clone(),
            };

            let silent_block_summaries = vec![
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: BlockItemSummaryDetails::AccountCreation(AccountCreationDetails {
                        credential_type: ArbitraryCredentialType::arbitrary(g).0,
                        address: receiver_address,
                        reg_id: CredentialRegistrationID::from_str("8a3a87f3f38a7a507d1e85dc02a92b8bcaa859f5cf56accb3c1bc7c40e1789b4933875a38dd4c0646ca3e940a02c42d8").unwrap(),
                    }),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: BlockItemSummaryDetails::Update(UpdateDetails {
                        effective_time: TransactionTime {
                            seconds: u64::arbitrary(g),
                        },
                        payload: UpdatePayload::EuroPerEnergy(ExchangeRate::new(7, 9).unwrap())
                    }),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: BlockItemSummaryDetails::AccountTransaction(account_transaction_details(AccountTransactionEffects::None {
                        reject_reason: RejectReason::OutOfEnergy,
                        transaction_type: Some(TransactionType::Update),
                    })),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: BlockItemSummaryDetails::AccountTransaction(account_transaction_details(AccountTransactionEffects::TransferredToEncrypted {
                        data: Box::new(EncryptedSelfAmountAddedEvent {
                            amount: amount.clone(),
                            account: receiver_address.clone(),
                            new_amount: EncryptedAmount {},
                        })
                        reject_reason: RejectReason::OutOfEnergy,
                        transaction_type: Some(TransactionType::Update),
                    })),
                }
            ];


            SilentBlockItemSummary(g.choose(&silent_block_summaries).unwrap().clone())
        }
    }
        #[derive(Clone, Debug)]
    enum BlockItemSummaryWrapper {
        Emitting(EmittingBlockItemSummaryPair),
        Silent(SilentBlockItemSummary),
    }

    impl Arbitrary for BlockItemSummaryWrapper {
        fn arbitrary(g: &mut Gen) -> Self {
            if bool::arbitrary(g) {
                BlockItemSummaryWrapper::Emitting(EmittingBlockItemSummaryPair::arbitrary(g))
            } else {
                BlockItemSummaryWrapper::Silent(SilentBlockItemSummary::arbitrary(g))
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
                BlockItemSummaryWrapper::Emitting(pair) => &pair.0,
                BlockItemSummaryWrapper::Silent(summary) => &summary.0,
            }
        }

        fn get_expected_notification(&self) -> Option<NotificationInformation> {
            match self {
                BlockItemSummaryWrapper::Emitting(pair) => Some(pair.1.clone()),
                BlockItemSummaryWrapper::Silent(_) => None,
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

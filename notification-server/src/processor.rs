use crate::models::notification::{
    CCDTransactionNotificationInformation, CIS2EventNotificationInformationBasic,
    NotificationInformationBasic, PLTEventNotificationInformation, PltAmount,
};
use concordium_rust_sdk::{
    base::hashes::TransactionHash,
    cis2::{self, Event},
    protocol_level_tokens,
    types::{self, AccountTransactionEffects, Address, ContractTraceElement},
};
use futures::{Stream, StreamExt};
use num_bigint::BigInt;

/// Extract basic Notification Information from the effects of an account
/// transaction.
fn map_transaction_to_notification_information(
    effects: &AccountTransactionEffects,
    transaction_hash: TransactionHash,
) -> anyhow::Result<Vec<NotificationInformationBasic>> {
    match &effects {
        AccountTransactionEffects::AccountTransfer { to, amount }
        | AccountTransactionEffects::AccountTransferWithMemo { to, amount, .. } => {
            Ok(vec![NotificationInformationBasic::CCD(
                CCDTransactionNotificationInformation {
                    address: *to,
                    amount: amount.micro_ccd.to_string(),
                    reference: transaction_hash,
                },
            )])
        }
        AccountTransactionEffects::ContractUpdateIssued { effects } => {
            let mut notifications = Vec::new();

            for effect in effects {
                let effect = effect.as_ref().known_or_err()?;

                let vec = match effect {
                    ContractTraceElement::Transferred { to, amount, .. } => {
                        vec![NotificationInformationBasic::CCD(
                            CCDTransactionNotificationInformation {
                                address: *to,
                                amount: amount.micro_ccd.to_string(),
                                reference: transaction_hash,
                            },
                        )]
                    }
                    ContractTraceElement::Updated { data } => {
                        let contract_address = data.address;
                        data.events
                            .iter()
                            .filter_map(|event| -> Option<CIS2EventNotificationInformationBasic> {
                                match cis2::Event::try_from(event).ok()? {
                                    Event::Transfer {
                                        token_id,
                                        to,
                                        amount,
                                        ..
                                    }
                                    | Event::Mint {
                                        token_id,
                                        owner: to,
                                        amount,
                                    } => {
                                        if let Address::Account(address) = to {
                                            Some(CIS2EventNotificationInformationBasic {
                                                address,
                                                amount: amount.0.to_string(),
                                                token_id,
                                                contract_address,
                                                reference: transaction_hash,
                                            })
                                        } else {
                                            None
                                        }
                                    }
                                    Event::Burn {
                                        token_id,
                                        owner,
                                        amount,
                                    } => {
                                        use std::ops::Neg;
                                        if let Address::Account(address) = owner {
                                            Some(CIS2EventNotificationInformationBasic {
                                                address,
                                                amount: BigInt::from(amount.0).neg().to_string(),
                                                token_id,
                                                contract_address,
                                                reference: transaction_hash,
                                            })
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                }
                            })
                            .map(NotificationInformationBasic::CIS2)
                            .collect()
                    }
                    _ => vec![],
                };
                notifications.extend(vec);
            }

            Ok(notifications)
        }

        AccountTransactionEffects::TransferredWithSchedule { to, amount }
        | AccountTransactionEffects::TransferredWithScheduleAndMemo { to, amount, .. } => {
            let amount: u64 = amount.iter().map(|(_, part)| part.micro_ccd()).sum();
            Ok(vec![NotificationInformationBasic::CCD(
                CCDTransactionNotificationInformation {
                    address: *to,
                    amount: amount.to_string(),
                    reference: transaction_hash,
                },
            )])
        }
        AccountTransactionEffects::TokenUpdate { events } => Ok(events
            .iter()
            .filter_map(|token_event| map_plt_token_events(transaction_hash, token_event))
            .map(NotificationInformationBasic::PLT)
            .collect()),
        _ => Ok(vec![]),
    }
}

/// Extract Notification information from a single PLT event.
fn map_plt_token_events(
    transaction_hash: TransactionHash,
    token_event: &protocol_level_tokens::TokenEvent,
) -> Option<PLTEventNotificationInformation> {
    use concordium_rust_sdk::protocol_level_tokens::TokenEventDetails;
    match &token_event.event {
        TokenEventDetails::Transfer(protocol_level_tokens::TokenTransferEvent {
            to,
            amount,
            ..
        }) => {
            let protocol_level_tokens::TokenHolder::Account { address } = to;
            Some(PLTEventNotificationInformation {
                address: *address,
                amount: PltAmount::from(*amount),
                token_id: token_event.token_id.clone(),
                reference: transaction_hash,
            })
        }
        TokenEventDetails::Mint(_) | TokenEventDetails::Burn(_) | TokenEventDetails::Module(_) => {
            None
        }
    }
}

pub async fn process(
    mut transactions: impl Stream<Item = Result<types::BlockItemSummary, tonic::Status>> + Unpin,
) -> anyhow::Result<Vec<NotificationInformationBasic>> {
    let mut notifications = Vec::new();

    while let Some(transaction) = transactions.next().await.transpose()? {
        let details = &transaction.details.known_or_err()?;

        let vec = match details {
            types::BlockItemSummaryDetails::AccountTransaction(account_transaction) => {
                let effects = account_transaction.effects.as_ref().known_or_err()?;

                map_transaction_to_notification_information(effects, transaction.hash)?
            }
            types::BlockItemSummaryDetails::TokenCreationDetails(details) => details
                .events
                .iter()
                .filter_map(|token_event| map_plt_token_events(transaction.hash, token_event))
                .map(NotificationInformationBasic::PLT)
                .collect(),
            _ => vec![],
        };

        notifications.extend(vec);
    }

    Ok(notifications)
}

#[cfg(test)]
mod tests {
    use crate::{
        models::notification::{
            CCDTransactionNotificationInformation, NotificationInformationBasic,
        },
        processor::process,
    };
    use concordium_rust_sdk::{
        base::{
            contracts_common::AccountAddress,
            elgamal::{Cipher, Message, PublicKey, SecretKey},
            hashes::HashBytes,
        },
        common::types::{Amount, Timestamp, TransactionTime},
        constants::EncryptedAmountsCurve,
        encrypted_transfers::types::EncryptedAmount,
        types::{
            hashes, AccountCreationDetails, AccountTransactionDetails, AccountTransactionEffects,
            BlockItemSummary, BlockItemSummaryDetails, CredentialRegistrationID, CredentialType,
            EncryptedSelfAmountAddedEvent, Energy, ExchangeRate, Memo, RejectReason,
            TransactionIndex, TransactionType, UpdateDetails, UpdatePayload,
        },
        v2::Upward,
    };
    use futures::stream;
    use quickcheck::{Arbitrary, Gen};
    use quickcheck_macros::quickcheck;
    use rand::{random, thread_rng, Rng};
    use sha2::Digest;
    use std::{fmt::Debug, str::FromStr};

    #[derive(Clone, Debug)]
    struct ArbitraryEnergy(pub Energy);

    impl Arbitrary for ArbitraryEnergy {
        fn arbitrary(g: &mut Gen) -> Self {
            ArbitraryEnergy(Energy {
                energy: u64::arbitrary(g),
            })
        }
    }

    #[derive(Clone, Debug)]
    struct ArbitraryCredentialType(pub CredentialType);
    impl Arbitrary for ArbitraryCredentialType {
        fn arbitrary(g: &mut Gen) -> Self {
            ArbitraryCredentialType(
                *g.choose(&[CredentialType::Initial, CredentialType::Normal])
                    .unwrap(),
            )
        }
    }

    fn random_transaction_index() -> TransactionIndex {
        TransactionIndex { index: random() }
    }

    fn random_memo() -> Memo {
        let mut rng = thread_rng();
        let size: usize = rng.gen_range(0..=256);
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

    /// Split the `ccd_amount` into `num_parts` and create a random release
    /// schedule timestamp for each of them. The parts are not of equal size and
    /// might even be of size 0.
    fn create_random_release_schedules_from_amount(
        mut amount: u64,
        max_elements: usize,
        g: &mut Gen,
    ) -> Vec<(Timestamp, Amount)> {
        let mut rng = thread_rng();
        let mut result = Vec::new();

        while amount > 0 && result.len() < max_elements {
            let part = rng.gen_range(0..=amount);
            result.push((u64::arbitrary(g).into(), Amount { micro_ccd: part }));
            amount -= part;
        }

        if amount > 0 {
            result.push((u64::arbitrary(g).into(), Amount { micro_ccd: amount }));
        }
        result
    }

    #[derive(Clone, Debug)]
    struct EmittingBlockItemSummaryPair(pub BlockItemSummary, pub NotificationInformationBasic);

    impl Arbitrary for EmittingBlockItemSummaryPair {
        fn arbitrary(g: &mut Gen) -> Self {
            let amount = Amount {
                micro_ccd: u64::arbitrary(g),
            };
            let receiver_address = AccountAddress(random_account_address());

            let details = |effects| AccountTransactionDetails {
                cost: amount,
                effects,
                sender: receiver_address,
            };
            let effects = vec![
                AccountTransactionEffects::AccountTransfer {
                    amount,
                    to: receiver_address,
                },
                AccountTransactionEffects::AccountTransferWithMemo {
                    amount,
                    to: receiver_address,
                    memo: random_memo(),
                },
                AccountTransactionEffects::TransferredWithSchedule {
                    to: receiver_address,
                    amount: create_random_release_schedules_from_amount(amount.micro_ccd, 2, g),
                },
                AccountTransactionEffects::TransferredWithScheduleAndMemo {
                    to: receiver_address,
                    amount: create_random_release_schedules_from_amount(amount.micro_ccd, 2, g),
                    memo: random_memo(),
                },
            ];

            let effect = g.choose(&effects).unwrap().clone();
            let hash = fixed_hash();
            let summary = BlockItemSummary {
                index: random_transaction_index(),
                energy_cost: ArbitraryEnergy::arbitrary(g).0,
                hash,
                details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(details(
                    Upward::Known(effect.clone()),
                ))),
            };

            let notification =
                NotificationInformationBasic::CCD(CCDTransactionNotificationInformation {
                    address: receiver_address,
                    amount: amount.micro_ccd().to_string(),
                    reference: hash,
                });

            EmittingBlockItemSummaryPair(summary, notification)
        }
    }

    #[derive(Clone, Debug)]
    struct SilentBlockItemSummary(pub BlockItemSummary);
    #[derive(Clone, Debug)]
    struct BlockItemSummaryWithUnkownVariant(pub BlockItemSummary);

    fn get_random_cipher() -> Cipher<EncryptedAmountsCurve> {
        let mut csprng = thread_rng();
        let sk: SecretKey<EncryptedAmountsCurve> = SecretKey::generate_all(&mut csprng);
        let pk = PublicKey::from(&sk);
        let m = Message::generate(&mut csprng);
        pk.encrypt(&mut csprng, &m)
    }

    impl Arbitrary for SilentBlockItemSummary {
        fn arbitrary(g: &mut Gen) -> Self {
            let amount = Amount {
                micro_ccd: u64::arbitrary(g),
            };
            let receiver_address = AccountAddress(random_account_address());

            let account_transaction_details = |effects| AccountTransactionDetails {
                cost: amount,
                effects,
                sender: receiver_address,
            };

            let silent_block_summaries = vec![
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details:Upward::Known( BlockItemSummaryDetails::AccountCreation(AccountCreationDetails {
                        credential_type: ArbitraryCredentialType::arbitrary(g).0,
                        address: receiver_address,
                        reg_id: CredentialRegistrationID::from_str("8a3a87f3f38a7a507d1e85dc02a92b8bcaa859f5cf56accb3c1bc7c40e1789b4933875a38dd4c0646ca3e940a02c42d8").unwrap(),
                    })),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details:Upward::Known( BlockItemSummaryDetails::Update(UpdateDetails {
                        effective_time: TransactionTime {
                            seconds: u64::arbitrary(g),
                        },
                        payload: Upward::Known(UpdatePayload::EuroPerEnergy(ExchangeRate::new(7, 9).unwrap()))
                    })),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details:Upward::Known( BlockItemSummaryDetails::AccountTransaction(account_transaction_details(Upward::Known(AccountTransactionEffects::None {
                        reject_reason: Upward::Known(RejectReason::OutOfEnergy),
                        transaction_type: Some(TransactionType::Update),
                    })))),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(account_transaction_details(Upward::Known(AccountTransactionEffects::TransferredToEncrypted {
                        data: Box::new(EncryptedSelfAmountAddedEvent {
                            amount,
                            account: receiver_address,
                            new_amount: EncryptedAmount {
                                encryptions: [get_random_cipher(), get_random_cipher()],
                            },
                        }),
                    })))),
                }
            ];

            SilentBlockItemSummary(g.choose(&silent_block_summaries).unwrap().clone())
        }
    }
    impl Arbitrary for BlockItemSummaryWithUnkownVariant {
        fn arbitrary(g: &mut Gen) -> Self {
            let amount = Amount {
                micro_ccd: u64::arbitrary(g),
            };
            let receiver_address = AccountAddress(random_account_address());

            let account_transaction_details = |effects| AccountTransactionDetails {
                cost: amount,
                effects,
                sender: receiver_address,
            };

            let block_summaries = vec![
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Unknown(()),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Known(BlockItemSummaryDetails::Update(UpdateDetails {
                        effective_time: TransactionTime {
                            seconds: u64::arbitrary(g),
                        },
                        payload: Upward::Unknown(()),
                    })),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Unknown(()),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(
                        account_transaction_details(Upward::Known(
                            AccountTransactionEffects::None {
                                reject_reason: Upward::Unknown(()),
                                transaction_type: Some(TransactionType::Update),
                            },
                        )),
                    )),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(
                        account_transaction_details(Upward::Unknown(())),
                    )),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Unknown(()),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Known(BlockItemSummaryDetails::AccountTransaction(
                        account_transaction_details(Upward::Unknown(())),
                    )),
                },
                BlockItemSummary {
                    index: random_transaction_index(),
                    energy_cost: ArbitraryEnergy::arbitrary(g).0,
                    hash: fixed_hash(),
                    details: Upward::Unknown(()),
                },
            ];

            BlockItemSummaryWithUnkownVariant(g.choose(&block_summaries).unwrap().clone())
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
        fn get_expected_notification(&self) -> Option<NotificationInformationBasic>;
    }

    impl TestableBlockItemSummary for BlockItemSummaryWrapper {
        fn get_block_item_summary(&self) -> &BlockItemSummary {
            match self {
                BlockItemSummaryWrapper::Emitting(pair) => &pair.0,
                BlockItemSummaryWrapper::Silent(summary) => &summary.0,
            }
        }

        fn get_expected_notification(&self) -> Option<NotificationInformationBasic> {
            match self {
                BlockItemSummaryWrapper::Emitting(pair) => Some(pair.1.clone()),
                BlockItemSummaryWrapper::Silent(_) => None,
            }
        }
    }

    #[quickcheck]
    fn test_random_block_item_summary(summaries: Vec<BlockItemSummaryWrapper>) -> bool {
        let summary_stream = stream::iter(
            summaries
                .clone()
                .into_iter()
                .map(|summary| Ok(summary.get_block_item_summary().clone()))
                .collect::<Vec<_>>(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(process(summary_stream));
        let result = match result {
            Ok(val) => val,
            Err(_) => {
                // Return/Fail test if an error occurs.
                return false;
            }
        };
        let expected: Vec<NotificationInformationBasic> = summaries
            .into_iter()
            .filter_map(|summary| summary.get_expected_notification())
            .collect();
        result == expected
    }

    #[quickcheck]
    fn test_random_block_item_summary_with_unkown_variant(
        summaries: Vec<BlockItemSummaryWithUnkownVariant>,
    ) {
        let summary_stream = stream::iter(
            summaries
                .clone()
                .into_iter()
                .map(|summary| Ok(summary.0.clone()))
                .collect::<Vec<_>>(),
        );
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(process(summary_stream));

        match result {
            Ok(_) => {}
            Err(e) => {
                let err_str = e.to_string();
                assert!(
                    err_str.contains("Encountered unknown data from the Node API on type"),
                    "Expected error message to contain 'unknown', but got: {}",
                    err_str
                );
            }
        }
    }
}

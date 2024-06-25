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

#[derive(Debug)]
pub struct NotificationInformation {
    pub address:  AccountAddress,
    pub amount:   BigInt,
    pub token_id: Option<TokenId>,
}

impl NotificationInformation {
    pub fn new(address: AccountAddress, amount: BigInt, token_id: Option<TokenId>) -> Self {
        Self {
            address,
            amount,
            token_id,
        }
    }
}

fn convert<T: Into<BigInt>>(
    address: Address,
    amount: T,
    is_positive: bool,
    token_id: TokenId,
) -> Option<NotificationInformation> {
    let mut amount: BigInt = amount.into();
    if !is_positive {
        amount = -amount;
    }

    match address {
        Account(address) => Some(NotificationInformation::new(
            address,
            amount,
            Some(token_id),
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
                None,
            )]
        }
        AccountTransactionEffects::AccountTransferWithMemo { to, amount, .. } => {
            vec![NotificationInformation::new(
                *to,
                amount.micro_ccd.into(),
                None,
            )]
        }
        AccountTransactionEffects::ContractUpdateIssued { effects } => effects
            .iter()
            .flat_map(|effect| match effect {
                ContractTraceElement::Transferred { to, amount, .. } => {
                    vec![NotificationInformation::new(
                        *to,
                        amount.micro_ccd.into(),
                        None,
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
                        }) => convert(to, amount.0, true, token_id),
                        Ok(Event::Mint {
                            owner,
                            amount,
                            token_id,
                        }) => convert(owner, amount.0, true, token_id),
                        Ok(Event::Burn {
                            owner,
                            amount,
                            token_id,
                        }) => convert(owner, amount.0, false, token_id),
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
                None,
            )]
        }
        AccountTransactionEffects::TransferredWithScheduleAndMemo { to, amount, .. } => {
            vec![NotificationInformation::new(
                *to,
                amount.iter().fold(BigInt::from(0), |acc, &(_, item)| {
                    acc + BigInt::from(item.micro_ccd)
                }),
                None,
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

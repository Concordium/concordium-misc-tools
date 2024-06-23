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
use std::fmt::Debug;

#[derive(Debug)]
pub struct NotificationInformation {
    effects: AccountAddress,
    amount:  BigInt,
}

impl NotificationInformation {
    pub fn new(effects: AccountAddress, amount: BigInt) -> Self { Self { effects, amount } }
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
        Account(address) => Some(NotificationInformation::new(address, amount)),
        _ => None,
    }
}

fn get_cis2_events_addresses(effects: &AccountTransactionEffects) -> Vec<NotificationInformation> {
    match &effects {
        AccountTransactionEffects::AccountTransfer { to, amount } => {
            vec![NotificationInformation::new(*to, amount.micro_ccd.into())]
        }
        AccountTransactionEffects::AccountTransferWithMemo { to, amount, .. } => {
            vec![NotificationInformation::new(*to, amount.micro_ccd.into())]
        }
        AccountTransactionEffects::ContractUpdateIssued { effects } => effects
            .iter()
            .flat_map(|effect| match effect {
                ContractTraceElement::Transferred { to, amount, .. } => {
                    vec![NotificationInformation::new(*to, amount.micro_ccd.into())]
                }
                ContractTraceElement::Updated { data } => data
                    .events
                    .iter()
                    .filter_map(|event| match cis2::Event::try_from(event) {
                        Ok(Event::Transfer { to, amount, .. }) => convert(to, amount.0, true),
                        Ok(Event::Mint { owner, amount, .. }) => convert(owner, amount.0, true),
                        Ok(Event::Burn { owner, amount, .. }) => convert(owner, amount.0, false),
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

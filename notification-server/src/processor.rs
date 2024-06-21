use concordium_rust_sdk::{
    cis2,
    cis2::Event,
    types,
    types::{
        AccountTransactionEffects, BlockItemSummaryDetails::AccountTransaction,
        ContractTraceElement,
    },
};
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use concordium_rust_sdk::types::Address::Account;
use futures::{Stream, StreamExt};

fn get_cis2_events_addresses(effects: &AccountTransactionEffects) -> Option<Vec<AccountAddress>> {
    match &effects {
        AccountTransactionEffects::ContractUpdateIssued { effects } => Some(
            effects
                .iter()
                .flat_map(|effect| match effect {
                    ContractTraceElement::Updated { data } => data
                        .events
                        .iter()
                        .map(|event| match cis2::Event::try_from(event) {
                            Ok(Event::Transfer { to, .. }) => Some(to),
                            Ok(Event::Mint { owner, .. }) => Some(owner),
                            _ => None,
                        })
                        .filter_map(|addr| match addr {
                            Some(Account(addr)) => Some(addr),
                            _ => None,
                        })
                        .collect(),
                    _ => vec![],
                })
                .collect(),
        ),
        _ => None,
    }
}

fn is_notification_emitting_transaction_effect(effects: &AccountTransactionEffects) -> bool {
    matches!(
        effects,
        AccountTransactionEffects::AccountTransfer { .. }
            | AccountTransactionEffects::AccountTransferWithMemo { .. }
            | AccountTransactionEffects::TransferredWithSchedule { .. }
            | AccountTransactionEffects::TransferredWithScheduleAndMemo { .. }
    )
}

pub async fn process(
    transactions: impl Stream<Item = Result<types::BlockItemSummary, tonic::Status>>,
) -> Vec<AccountAddress> {
    transactions
        .filter_map(|result| async move { result.ok() })
        .filter_map(|t| async move {
            match t.details {
                AccountTransaction(ref account_transaction) => {
                    if is_notification_emitting_transaction_effect(&account_transaction.effects) {
                        Some(
                            t.affected_addresses()
                                .into_iter()
                                .map(|addr| addr)
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        get_cis2_events_addresses(&account_transaction.effects)
                    }
                }
                _ => None,
            }
        })
        .collect::<Vec<Vec<AccountAddress>>>()
        .await
        .into_iter()
        .flatten()
        .collect()
}

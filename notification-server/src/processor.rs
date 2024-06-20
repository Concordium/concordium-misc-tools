use concordium_rust_sdk::{
    cis2,
    cis2::Event,
    types,
    types::{
        AccountTransactionEffects, BlockItemSummaryDetails::AccountTransaction,
        ContractTraceElement,
    },
};
use futures::{Stream, StreamExt};

fn get_cis2_events_addresses(effects: &AccountTransactionEffects) -> Option<Vec<String>> {
    match &effects {
        AccountTransactionEffects::ContractUpdateIssued { effects } => Some(
            effects
                .iter()
                .flat_map(|effect| match effect {
                    ContractTraceElement::Updated { data } => data
                        .events
                        .iter()
                        .map(|event| match cis2::Event::try_from(event) {
                            Ok(Event::Transfer { to, .. }) => Some(to.to_string()),
                            Ok(Event::Mint { amount, .. }) => Some(amount.to_string()),
                            _ => None,
                        })
                        .filter(Option::is_some)
                        .collect(),
                    _ => None,
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
) -> Vec<String> {
    transactions
        .filter_map(|result| async move { result.ok() })
        .filter_map(|t| async move {
            match t.details {
                AccountTransaction(ref account_transaction) => {
                    if is_notification_emitting_transaction_effect(&account_transaction.effects) {
                        Some(
                            t.affected_addresses()
                                .into_iter()
                                .map(|addr| addr.to_string())
                                .collect::<Vec<_>>(),
                        )
                    } else {
                        get_cis2_events_addresses(&account_transaction.effects)
                    }
                }
                _ => None,
            }
        })
        .collect::<Vec<Vec<String>>>()
        .await
        .into_iter()
        .flatten()
        .collect()
}

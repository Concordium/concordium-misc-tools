use concordium_rust_sdk::base::contracts_common::AccountAddress;
use enum_iterator::Sequence;
use num_bigint::BigInt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents details for a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotificationInformation {
    /// The blockchain account address unawarely involved in the notification
    /// emitting event.
    pub address: AccountAddress,
    /// The amount being involved in the notification emitting event.
    pub amount:  BigInt,
}

impl NotificationInformation {
    pub fn new(address: AccountAddress, amount: BigInt) -> Self { Self { address, amount } }
}

impl NotificationInformation {
    pub fn into_hashmap(self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("amount".to_string(), self.amount.to_string());
        map.insert("recipient".to_string(), self.address.to_string());
        map
    }
}

#[derive(Debug, Deserialize)]
pub struct DeviceSubscription {
    pub preferences: Vec<Preference>,
    pub accounts:    Vec<String>,
}

impl DeviceSubscription {
    pub fn new(preferences: Vec<Preference>, accounts: Vec<String>) -> Self {
        Self {
            preferences,
            accounts,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize, Sequence)]
#[serde(rename_all = "PascalCase")]
pub enum Preference {
    #[serde(rename = "cis2-tx")]
    CIS2Transaction,
    #[serde(rename = "ccd-tx")]
    CCDTransaction,
}

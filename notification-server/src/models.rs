use std::collections::HashSet;
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};

/// Represents details for a notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NotificationInformation {
    /// The blockchain account address unawarely involved in the notification
    /// emitting event.
    #[serde(rename = "recipient")]
    pub address:          AccountAddress,
    /// The amount being involved in the notification emitting event.
    pub amount:           String,
    /// The type of event that the notification is about.
    #[serde(rename = "type")]
    pub transaction_type: Preference,
}

impl NotificationInformation {
    pub fn new(address: AccountAddress, amount: String, transaction_type: Preference) -> Self {
        Self {
            address,
            amount,
            transaction_type,
        }
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

#[derive(Debug, Deserialize)]
pub struct Device {
    pub preferences: HashSet<Preference>,
    pub device_token: String,
}

impl Device {
    pub fn new(preferences: HashSet<Preference>, device_token: String) -> Self {
        Self { preferences, device_token }
    }
}

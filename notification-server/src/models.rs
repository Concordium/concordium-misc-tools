use concordium_rust_sdk::base::contracts_common::AccountAddress;
use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;


#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, )]
pub enum NotificationInformationType {
    CCD(CCDTransactionNotificationInformation),
    CIS2(CIS2EventNotificationInformation),
}

pub trait NotificationInformation: Serialize {
    /// The blockchain account address unawarely involved in the notification
    /// emitting event.
    fn address(&self) -> &AccountAddress;
    /// The amount being involved in the notification emitting event.
    fn amount(&self) -> &String;
    /// The type of event that the notification is about.
    fn transaction_type(&self) -> &Preference;
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CCDTransactionNotificationInformation {
    #[serde(rename = "recipient")]
    pub address: AccountAddress,
    pub amount: String,
    #[serde(rename = "type")]
    pub transaction_type: Preference
}

impl CCDTransactionNotificationInformation {
    pub fn new(address: AccountAddress, amount: String) -> Self {
        Self { address, amount, transaction_type: Preference::CCDTransaction }
    }
}


#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CIS2EventNotificationInformation {
    #[serde(rename = "recipient")]
    pub address: AccountAddress,
    pub amount: String,
    #[serde(rename = "type")]
    pub transaction_type: Preference,
}

impl CIS2EventNotificationInformation {
    pub fn new(address: AccountAddress, amount: String) -> Self {
        Self { address, amount, transaction_type: Preference::CIS2Transaction }
    }
}

impl NotificationInformation for NotificationInformationType {
    fn address(&self) -> &AccountAddress {
        match self {
            NotificationInformationType::CCD(info) => info.address(),
            NotificationInformationType::CIS2(info) => info.address(),
        }
    }

    fn amount(&self) -> &String {
        match self {
            NotificationInformationType::CCD(info) => info.amount(),
            NotificationInformationType::CIS2(info) => info.amount(),
        }
    }

    fn transaction_type(&self) -> &Preference {
        match self {
            NotificationInformationType::CCD(info) => info.transaction_type(),
            NotificationInformationType::CIS2(info) => info.transaction_type(),
        }
    }
}

impl NotificationInformation for CCDTransactionNotificationInformation {
    fn address(&self) -> &AccountAddress {
        &self.address
    }

    fn amount(&self) -> &String {
        &self.amount
    }

    fn transaction_type(&self) -> &Preference {
        &self.transaction_type
    }
}

impl NotificationInformation for CIS2EventNotificationInformation {
    fn address(&self) -> &AccountAddress {
        &self.address
    }

    fn amount(&self) -> &String {
        &self.amount
    }

    fn transaction_type(&self) -> &Preference {
        &self.transaction_type
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
    pub preferences:  HashSet<Preference>,
    pub device_token: String,
}

impl Device {
    pub fn new(preferences: HashSet<Preference>, device_token: String) -> Self {
        Self {
            preferences,
            device_token,
        }
    }
}

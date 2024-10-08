use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Deserialize)]
pub struct DeviceSubscription {
    pub preferences:  Vec<Preference>,
    pub accounts:     Vec<String>,
    pub device_token: String,
}

impl DeviceSubscription {
    pub fn new(preferences: Vec<Preference>, accounts: Vec<String>, device_token: String) -> Self {
        Self {
            preferences,
            accounts,
            device_token,
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

#[derive(Debug, Deserialize, PartialEq)]
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

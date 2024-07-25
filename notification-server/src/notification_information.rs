use std::collections::HashMap;
use concordium_rust_sdk::base::contracts_common::AccountAddress;
use num_bigint::BigInt;

#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct NotificationInformation {
    pub address: AccountAddress,
    pub amount:  BigInt,
}

impl NotificationInformation {
    pub fn new(address: AccountAddress, amount: BigInt) -> Self { Self { address, amount } }
}

impl NotificationInformation {
    pub fn into_hashmap(self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map
    }
}

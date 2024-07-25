use concordium_rust_sdk::base::contracts_common::AccountAddress;
use num_bigint::BigInt;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
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
        map.insert("amount".to_string(), self.amount.to_string());
        map.insert("sender".to_string(), self.address.to_string());
        HashMap::new()
    }
}

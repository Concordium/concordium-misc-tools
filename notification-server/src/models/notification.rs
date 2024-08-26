use crate::models::device::Preference;
use concordium_rust_sdk::{
    base::{contracts_common::AccountAddress, smart_contracts::OwnedContractName},
    cis2::{Cis2QueryError, Cis2Type, MetadataUrl, TokenId},
    contract_client::ContractClient,
    types::{hashes::TransactionHash, ContractAddress},
    v2::{Client, IntoBlockIdentifier},
};
use serde::{Deserialize, Serialize, Serializer};
use std::ops::Deref;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationInformationBasic {
    CCD(CCDTransactionNotificationInformation),
    CIS2(CIS2EventNotificationInformationBasic),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationInformation {
    CCD(CCDTransactionNotificationInformation),
    CIS2(CIS2EventNotificationInformation),
}

impl Serialize for NotificationInformation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer, {
        match self {
            NotificationInformation::CCD(inner) => inner.serialize(serializer),
            NotificationInformation::CIS2(inner) => inner.serialize(serializer),
        }
    }
}

impl NotificationInformationBasic {
    pub async fn enrich<T: IntoBlockIdentifier>(
        self,
        client: Client,
        block_identifier: T,
    ) -> Result<NotificationInformation, Cis2QueryError> {
        match self {
            NotificationInformationBasic::CCD(info) => Ok(NotificationInformation::CCD(info)),
            NotificationInformationBasic::CIS2(info) => {
                let mut contract_client: ContractClient<Cis2Type> =
                    ContractClient::create(client, info.contract_address).await?;
                Ok(NotificationInformation::CIS2(
                    CIS2EventNotificationInformation::new(
                        info.address,
                        info.amount,
                        info.token_id.clone(),
                        info.contract_address,
                        contract_client.contract_name.deref().clone(),
                        contract_client
                            .token_metadata_single(block_identifier, info.token_id)
                            .await
                            .ok(),
                        info.transaction_hash,
                    ),
                ))
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CCDTransactionNotificationInformation {
    #[serde(rename = "recipient")]
    pub address:          AccountAddress,
    pub amount:           String,
    #[serde(rename = "type")]
    pub transaction_type: Preference,
    pub reference:        TransactionHash,
}

impl CCDTransactionNotificationInformation {
    pub fn new(address: AccountAddress, amount: String, reference: TransactionHash) -> Self {
        Self {
            address,
            amount,
            transaction_type: Preference::CCDTransaction,
            reference,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CIS2EventNotificationInformation {
    #[serde(rename = "recipient")]
    pub address:          AccountAddress,
    pub amount:           String,
    #[serde(rename = "type")]
    pub transaction_type: Preference,
    pub token_id:         TokenId,
    pub contract_address: ContractAddress,
    pub contract_name:    OwnedContractName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_metadata:   Option<MetadataUrl>,
    pub reference:        TransactionHash,
}

impl CIS2EventNotificationInformation {
    pub fn new(
        address: AccountAddress,
        amount: String,
        token_id: TokenId,
        contract_address: ContractAddress,
        contract_name: OwnedContractName,
        token_metadata_url: Option<MetadataUrl>,
        reference: TransactionHash,
    ) -> Self {
        Self {
            address,
            amount,
            transaction_type: Preference::CIS2Transaction,
            token_id,
            contract_address,
            contract_name,
            token_metadata: token_metadata_url,
            reference,
        }
    }
}

impl NotificationInformationBasic {
    pub fn address(&self) -> &AccountAddress {
        match self {
            NotificationInformationBasic::CCD(info) => &info.address,
            NotificationInformationBasic::CIS2(info) => &info.address,
        }
    }

    pub fn amount(&self) -> &str {
        match self {
            NotificationInformationBasic::CCD(info) => &info.amount,
            NotificationInformationBasic::CIS2(info) => &info.amount,
        }
    }

    pub fn transaction_type(&self) -> &Preference {
        match self {
            NotificationInformationBasic::CCD(info) => &info.transaction_type,
            NotificationInformationBasic::CIS2(info) => &info.transaction_type,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CIS2EventNotificationInformationBasic {
    address:          AccountAddress,
    amount:           String,
    transaction_type: Preference,
    token_id:         TokenId,
    contract_address: ContractAddress,
    transaction_hash: TransactionHash,
}

impl CIS2EventNotificationInformationBasic {
    pub fn new(
        address: AccountAddress,
        amount: String,
        token_id: TokenId,
        contract_address: ContractAddress,
        transaction_hash: TransactionHash,
    ) -> Self {
        Self {
            address,
            amount,
            transaction_type: Preference::CIS2Transaction,
            token_id,
            contract_address,
            transaction_hash,
        }
    }
}

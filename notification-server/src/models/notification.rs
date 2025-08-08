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

/// Notification information directly parsed from the block item summary.
///
/// Note: For some instances additional data needs to be fetched, before it can
/// be converted to a [`NotificationInformation`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NotificationInformationBasic {
    CCD(CCDTransactionNotificationInformation),
    CIS2(CIS2EventNotificationInformationBasic),
}

/// Data transmitted in a notification.
///
/// This will be serialized into JSON when submitting the notification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "type")]
pub enum NotificationInformation {
    #[serde(rename = "ccd-tx")]
    CCD(CCDTransactionNotificationInformation),
    #[serde(rename = "cis2-tx")]
    CIS2(CIS2EventNotificationInformation),
}

impl NotificationInformationBasic {
    /// Convert into a [`NotificationInformation`] by fetching the missing
    /// information from the Concordium Node.
    pub async fn enrich(
        self,
        client: Client,
        block_identifier: impl IntoBlockIdentifier,
    ) -> Result<NotificationInformation, Cis2QueryError> {
        match self {
            NotificationInformationBasic::CCD(info) => Ok(NotificationInformation::CCD(info)),
            NotificationInformationBasic::CIS2(info) => {
                let mut contract_client: ContractClient<Cis2Type> =
                    ContractClient::create(client, info.contract_address).await?;
                let contract_name = contract_client.contract_name.deref().clone();
                let token_metadata = contract_client
                    .token_metadata_single(block_identifier, info.token_id.clone())
                    .await
                    .ok();
                Ok(NotificationInformation::CIS2(
                    CIS2EventNotificationInformation {
                        info,
                        contract_name,
                        token_metadata,
                    },
                ))
            }
        }
    }
}

/// CCD related notification information.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CCDTransactionNotificationInformation {
    /// Address of the account receiving some amount of CCD.
    #[serde(rename = "recipient")]
    pub address:   AccountAddress,
    /// String encoding of microCCD.
    pub amount:    String,
    /// Hash of the transaction which caused the notification.
    pub reference: TransactionHash,
}

/// CIS-2 related notification information.
///
/// Note: this includes additional information that what can be directly
/// extracted from the block item summary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CIS2EventNotificationInformation {
    /// Name of the token smart contract.
    #[serde(serialize_with = "serialize_contract_name")]
    pub contract_name:  OwnedContractName,
    /// Metadata URL for the token.
    #[serde(
        serialize_with = "serialize_option_as_json_string",
        skip_serializing_if = "Option::is_none"
    )]
    pub token_metadata: Option<MetadataUrl>,
    /// The basic information extracted directly from the block item summary.
    #[serde(flatten)]
    pub info:           CIS2EventNotificationInformationBasic,
}

fn serialize_contract_name<S>(name: &OwnedContractName, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer, {
    let contract_name_str = name.as_contract_name();
    serializer.serialize_str(contract_name_str.contract_name())
}

fn serialize_as_json_string<S, T>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize, {
    let json_string = serde_json::to_string(value).map_err(serde::ser::Error::custom)?;
    serializer.serialize_str(&json_string)
}

fn serialize_option_as_json_string<S, T>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    T: Serialize, {
    match value {
        Some(v) => serialize_as_json_string(v, serializer),
        None => serializer.serialize_none(),
    }
}

impl NotificationInformationBasic {
    /// Get the affected account address.
    pub fn address(&self) -> &AccountAddress {
        match self {
            NotificationInformationBasic::CCD(info) => &info.address,
            NotificationInformationBasic::CIS2(info) => &info.address,
        }
    }

    /// Get the [`Preference`] related to this type of notification.
    pub fn preference(&self) -> Preference {
        match self {
            NotificationInformationBasic::CCD(_) => Preference::CCDTransaction,
            NotificationInformationBasic::CIS2(_) => Preference::CIS2Transaction,
        }
    }
}

/// Notification information extracted directly from the block item summary.
///
/// Note that additional information needs to be fetched to be converted to a
/// [`CIS2EventNotificationInformation`]
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CIS2EventNotificationInformationBasic {
    /// Account address receiving some CIS-2 token.
    #[serde(rename = "recipient")]
    pub address:          AccountAddress,
    /// String encoding of the integer token amount.
    /// Will be negative when emitted due to a burn.
    pub amount:           String,
    /// The token identifier within the smart contract.
    pub token_id:         TokenId,
    /// The contract address of the token.
    #[serde(serialize_with = "serialize_as_json_string")]
    pub contract_address: ContractAddress,
    /// Hash of the transaction which cause the notification.
    pub reference:        TransactionHash,
}

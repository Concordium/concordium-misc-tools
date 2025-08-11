use crate::models::device::Preference;
use concordium_rust_sdk::{
    base::{contracts_common::AccountAddress, smart_contracts::OwnedContractName},
    cis2::{Cis2QueryError, Cis2Type, MetadataUrl, TokenId},
    contract_client::ContractClient,
    protocol_level_tokens,
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
    PLT(PLTEventNotificationInformation),
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
    #[serde(rename = "plt-tx")]
    PLT(PLTEventNotificationInformation),
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
            NotificationInformationBasic::PLT(info) => Ok(NotificationInformation::PLT(info)),
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
            NotificationInformationBasic::PLT(info) => &info.address,
        }
    }

    /// Get the [`Preference`] related to this type of notification.
    pub fn preference(&self) -> Preference {
        match self {
            NotificationInformationBasic::CCD(_) => Preference::CCDTransaction,
            NotificationInformationBasic::CIS2(_) => Preference::CIS2Transaction,
            NotificationInformationBasic::PLT(_) => Preference::PLTTransaction,
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

/// Notification information related to protocol-level tokens (PLT).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PLTEventNotificationInformation {
    /// Account address receiving some PLT token.
    #[serde(rename = "recipient")]
    pub address:   AccountAddress,
    /// String encoding of the integer token amount.
    #[serde(flatten)]
    pub amount:    protocol_level_tokens::TokenAmount,
    /// The token identifier within the smart contract.
    pub token_id:  protocol_level_tokens::TokenId,
    /// Hash of the transaction which cause the notification.
    pub reference: TransactionHash,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// Test case to ensure changes to the JSON serialization are caught.
    #[test]
    fn test_serialize_notification_information() {
        // CCD notification
        {
            let notification_information =
                NotificationInformation::CCD(CCDTransactionNotificationInformation {
                    address:   "4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp"
                        .parse()
                        .unwrap(),
                    amount:    "100".to_string(),
                    reference: "3d1c2f4fb9a0eb468bfe39e75c59897c1a375082a6440f4a5da77102182ba055"
                        .parse()
                        .unwrap(),
                });

            let expected = json!({
                "type": "ccd-tx",
                "amount": "100",
                "recipient": "4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp",
                "reference": "3d1c2f4fb9a0eb468bfe39e75c59897c1a375082a6440f4a5da77102182ba055"
            });
            assert_eq!(
                expected,
                serde_json::to_value(notification_information).unwrap()
            )
        }

        // CIS-2 notification
        {
            let notification_information =
                NotificationInformation::CIS2(CIS2EventNotificationInformation {
                    contract_name:  OwnedContractName::new("init_contract".to_string()).unwrap(),
                    token_metadata: Some(
                        MetadataUrl::new("https://example.com".to_string(), None).unwrap(),
                    ),
                    info:           CIS2EventNotificationInformationBasic {
                        address:          "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G"
                            .parse()
                            .unwrap(),
                        amount:           "200".to_string(),
                        token_id:         "ffffff".parse().unwrap(),
                        contract_address: ContractAddress::new(112, 2),
                        reference:
                            "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110"
                                .parse()
                                .unwrap(),
                    },
                });

            let expected = json!({
                "type": "cis2-tx",
                "amount": "200",
                "recipient": "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G",
                "token_id": "ffffff",
                "reference": "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110",
                "token_metadata": "{\"url\":\"https://example.com\",\"hash\":null}",
                "contract_address": "{\"index\":112,\"subindex\":2}",
                "contract_name": "contract"
            });
            assert_eq!(
                expected,
                serde_json::to_value(notification_information).unwrap()
            )
        }

        // PLT notification
        {
            let notification_information =
                NotificationInformation::PLT(PLTEventNotificationInformation {
                    address:   "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G"
                        .parse()
                        .unwrap(),
                    amount:    protocol_level_tokens::TokenAmount::from_raw(123456789, 6),
                    token_id:  "TestCoin".parse().unwrap(),
                    reference: "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110"
                        .parse()
                        .unwrap(),
                });

            let expected = json!({
                "type": "plt-tx",
                "token_id": "TestCoin",
                "recipient": "3kBx2h5Y2veb4hZgAJWPrr8RyQESKm5TjzF3ti1QQ4VSYLwK1G",
                "decimals": 6,
                "value": "123456789",
                "reference": "494d7848e389d44a2c2fe81eeee6dc427ce33ab1d0c92cba23be321d495be110",
            });
            assert_eq!(
                expected,
                serde_json::to_value(notification_information).unwrap()
            )
        }
    }
}

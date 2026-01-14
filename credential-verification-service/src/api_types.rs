use concordium_rust_sdk::base::web3id::v1::anchor::PresentationVerifyFailure;
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::{
    base::{
        hashes::TransactionHash,
        web3id::v1::{
            PresentationV1,
            anchor::{RequestedSubjectClaims, VerificationAuditRecord, VerificationRequest},
        },
    },
    id::constants::{ArCurve, IpPairing},
    web3id::Web3IdAttribute,
};
use std::collections::HashMap;

/// Parameters posted to this service when calling the API
/// endpoint `/verifiable-presentations/create-verification-request`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateVerificationRequest {
    /// An identifier for some connection (e.g. wallet-connect topic) included in the verification request context.
    pub connection_id: String,
    /// A resource id to track the connected website (e.g. website URL or TLS fingerprint).
    pub resource_id: String,
    /// A general purpose string value included in the verification request context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_string: Option<String>,
    /// The subject claims being requested to be proven.
    pub requested_claims: Vec<RequestedSubjectClaims>,
    /// Additional public info which will be included in the anchor transaction (VRA)
    /// that is submitted on-chain.
    #[serde(
        with = "map_hex_cbor_values_option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub public_info: Option<HashMap<String, cbor::value::Value>>,
}

/// API request payload for verifying a presentation
/// endpoint: `/verifiable-presentations/verify`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VerifyPresentationRequest {
    /// Audit record id that the client wants to include in the audit anchor.
    pub audit_record_id: String,
    /// Additional public info which will be included in the anchor transaction (VAA)
    /// that is submitted on-chain.
    #[serde(
        with = "map_hex_cbor_values_option",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub public_info: Option<HashMap<String, cbor::value::Value>>,
    /// Verifiable presentation that contains verifiable credentials each
    /// consisting of subject claims and proofs of them.
    /// It is the response to proving a [`RequestV1`] with [`RequestV1::prove`].
    pub presentation: PresentationV1<IpPairing, ArCurve, Web3IdAttribute>,
    /// A verification request that specifies which subject claims are requested from a credential holder
    /// and in which context.
    pub verification_request: VerificationRequest,
}

/// Response to verifying a presentation
/// endpoint: `/verifiable-presentations/verify`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct VerifyPresentationResponse {
    /// Whether the verification was successful or not for the presentation
    pub result: VerificationResult,
    /// Audit record which contains the complete verified request and presentation
    pub verification_audit_record: VerificationAuditRecord,
    /// Audit anchor transaction hash reference that was put on chain
    pub anchor_transaction_hash: Option<TransactionHash>,
}

/// Presentation Verification Result, contains: Success or Failed with a String message
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum VerificationResult {
    /// Verified
    Verified,
    /// Failed with reason for the verification failing
    Failed(VerificationFailure),
}

/// Representation of why a presentation did not verify
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VerificationFailure {
    /// Reason presentation did not verify
    pub code: PresentationVerifyFailure,
    /// User displayable message of why presentation did not verify
    pub message: String,
}

/// Definition of Error Response structure to be sent back to the client.
pub struct ErrorResponse {
    /// The body of the error
    pub error: ErrorBody,
}

pub struct ErrorBody {
    /// machine readable error code that has occurred. All uppercase wording separated by underscore.
    /// eg: VALIDATION_ERROR
    pub code: String,
    /// high level error message descrbing the error
    pub message: String,
    /// request trace id for distributed logging
    pub trace_id: String,
    pub retryable: bool,
    pub details: Vec<ErrorDetail>,
}

/// A specific Error detail that has occurred
pub struct ErrorDetail {
    /// machine readable error code about a specific error that has occurred
    pub code: String,
    /// path of the problem. This could be a request payload path to a specific field causing the issue.
    pub path: String,
    /// specific helpful error message defining what has happened for this error
    pub message: String,
}

mod map_hex_cbor_values_option {
    use super::*;
    use hex::{FromHex, ToHex};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    /// Serialize a `HashMap<String, value::Value>` as hex-encoded CBOR.
    pub fn serialize<S>(
        map: &Option<HashMap<String, cbor::value::Value>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut hex_map = HashMap::new();
        for (key, value) in map.iter().flatten() {
            let cbor_bytes = cbor::cbor_encode(value).map_err(serde::ser::Error::custom)?;
            hex_map.insert(key, cbor_bytes.encode_hex::<String>());
        }
        hex_map.serialize(serializer)
    }

    /// Deserialize a `HashMap<String, value::Value>` from hex-encoded CBOR.
    pub fn deserialize<'de, D>(
        deserializer: D,
    ) -> Result<Option<HashMap<String, cbor::value::Value>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex_map: HashMap<String, String> = HashMap::deserialize(deserializer)?;
        let mut map = HashMap::new();
        for (key, hex_str) in hex_map {
            let cbor_bytes = Vec::from_hex(&hex_str).map_err(serde::de::Error::custom)?;
            let value = cbor::cbor_decode(&cbor_bytes).map_err(serde::de::Error::custom)?;
            map.insert(key, value);
        }
        Ok(Some(map))
    }
}

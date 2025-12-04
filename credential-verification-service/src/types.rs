use axum::{Json, http::StatusCode};
use concordium_rust_sdk::{
    common::cbor::value::Value as BaseCborValue,
    types::{Nonce, WalletAccount},
    v2,
    web3id::{did::Network, v1::CreateAnchorError},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer, ser::Error as SerError};
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Holds the service state in memory.
///
/// Note: A new instance of this struct is created whenever the service restarts.
pub struct Service {
    /// The client to interact with the node.
    pub node_client: v2::Client,
    /// The network of the connected node.  
    pub network: Network,
    /// The key and address of the account submitting the anchor transactions on-chain.
    pub account_keys: Arc<WalletAccount>,
    /// The current nonce of the account submitting the anchor transactions on-chain.
    pub nonce: Arc<Mutex<Nonce>>,
    /// The number of seconds in the future when the anchor transactions should expiry.  
    pub transaction_expiry_secs: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("Unable to submit anchor transaction on chain: {0}.")]
    SubmitAnchorTransaction(#[from] CreateAnchorError),
}

impl axum::response::IntoResponse for ServerError {
    fn into_response(self) -> axum::response::Response {
        let r = match self {
            ServerError::SubmitAnchorTransaction(error) => {
                tracing::error!("Internal error: {error}.");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json("Internal error.".to_string()),
                )
            }
        };
        r.into_response()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CborValue(pub BaseCborValue);

fn string_key_from_cbor(k: &BaseCborValue) -> Result<String, String> {
    match k {
        BaseCborValue::Text(t) => Ok(t.clone()),
        BaseCborValue::Positive(n) => Ok(n.to_string()),
        BaseCborValue::Negative(n) => Ok(n.to_string()),
        BaseCborValue::Bool(b) => Ok(b.to_string()),
        BaseCborValue::Null => Ok("null".into()),
        BaseCborValue::Float(f) => Ok(f.to_string()),
        BaseCborValue::Bytes(bytes) => Ok(format!("0x{}", hex::encode(&bytes.0))),
        // We don't support arrays/maps/tags as keys in maps in this service.
        BaseCborValue::Array(_) | BaseCborValue::Map(_) | BaseCborValue::Tag(_, _) => {
            Err("CBOR array/map/tags are not supported as key in maps in this service.".into())
        }
        BaseCborValue::Simple(n) => Ok(n.to_string()),
    }
}

fn json_from_cbor<E>(v: &BaseCborValue) -> Result<JsonValue, E>
where
    E: SerError,
{
    match v {
        BaseCborValue::Positive(n) => Ok(JsonValue::from(*n)),
        BaseCborValue::Negative(n) => Ok(JsonValue::from(*n)),
        BaseCborValue::Bytes(bytes) => Ok(JsonValue::String(hex::encode(&bytes.0))),
        BaseCborValue::Text(text) => Ok(JsonValue::String(text.clone())),

        BaseCborValue::Array(arr) => {
            let items = arr
                .iter()
                .map(json_from_cbor)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(JsonValue::Array(items))
        }

        BaseCborValue::Map(map) => {
            let obj = map
                .iter()
                .map(|(k, v)| {
                    let key = string_key_from_cbor(k).map_err(SerError::custom)?;
                    let val = json_from_cbor(v)?;
                    Ok((key, val))
                })
                .collect::<Result<serde_json::Map<_, _>, _>>()?;
            Ok(JsonValue::Object(obj))
        }

        BaseCborValue::Tag(tag, inner) => Ok(serde_json::json!({
            "tag": tag,
            "value": json_from_cbor(inner)?,
        })),

        BaseCborValue::Bool(b) => Ok(JsonValue::Bool(*b)),
        BaseCborValue::Null => Ok(JsonValue::Null),
        BaseCborValue::Float(f) => Ok(JsonValue::from(*f)),
        BaseCborValue::Simple(n) => Ok(JsonValue::from(*n)),
    }
}

impl Serialize for CborValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        json_from_cbor::<S::Error>(&self.0)?.serialize(serializer)
    }
}

fn cbor_from_json(v: JsonValue) -> Result<BaseCborValue, String> {
    match v {
        JsonValue::Null => Ok(BaseCborValue::Null),
        JsonValue::Bool(b) => Ok(BaseCborValue::Bool(b)),
        JsonValue::Number(n) => {
            if let Some(u) = n.as_u64() {
                Ok(BaseCborValue::Positive(u))
            } else if let Some(i) = n.as_i64() {
                Ok(BaseCborValue::Negative(i.try_into().unwrap()))
            } else if let Some(f) = n.as_f64() {
                Ok(BaseCborValue::Float(f))
            } else {
                Err("Unsupported JSON number".into())
            }
        }
        JsonValue::String(s) => Ok(BaseCborValue::Text(s)),
        JsonValue::Array(arr) => {
            let items = arr
                .into_iter()
                .map(cbor_from_json)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(BaseCborValue::Array(items))
        }
        JsonValue::Object(obj) => {
            let mut map = Vec::new();

            for (k, v) in obj {
                // JSON keys always become CBOR text
                let key = BaseCborValue::Text(k);
                let val = cbor_from_json(v)?;
                map.push((key, val));
            }

            Ok(BaseCborValue::Map(map))
        }
    }
}

impl<'de> Deserialize<'de> for CborValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let json_value = JsonValue::deserialize(deserializer)?;
        let cbor_value = cbor_from_json(json_value).map_err(serde::de::Error::custom)?;
        Ok(CborValue(cbor_value))
    }
}

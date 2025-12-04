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
        BaseCborValue::Negative(n) => Ok(format!("-{}", n)),
        BaseCborValue::Bool(b) => Ok(b.to_string()),
        BaseCborValue::Null => Ok("null".into()),
        BaseCborValue::Float(f) => Ok(f.to_string()),
        BaseCborValue::Bytes(bytes) => Ok(format!("0x{}", hex::encode(&bytes.0))),
        // We don't support arrays/maps/tags as keys in maps in this service.
        BaseCborValue::Array(_) | BaseCborValue::Map(_) | BaseCborValue::Tag(_, _) => {
            Err("CBOR array/map/tags are not supported as keys in maps in this service.".into())
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
        BaseCborValue::Negative(n) => {
            let val: i128 = -(*n as i128);
            Ok(JsonValue::from(val))
        }
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
            } else if let Some(i) = n.as_i128() {
                // Must be negative
                if i < 0 && i.abs() <= u64::MAX as i128 {
                    let encoded = -i as u64;
                    Ok(BaseCborValue::Negative(encoded))
                } else if let Some(f) = n.as_f64() {
                    Ok(BaseCborValue::Float(f))
                } else {
                    Err("Unsupported JSON number".into())
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Number, json};

    // Helper: roundtrip CBOR -> JSON -> CBOR
    fn roundtrip_cbor(c: BaseCborValue) -> BaseCborValue {
        let json = json_from_cbor::<serde_json::Error>(&c).unwrap();
        cbor_from_json(json).unwrap()
    }

    // Helper: roundtrip JSON -> CBOR -> JSON
    fn roundtrip_json(j: serde_json::Value) -> serde_json::Value {
        let cbor = cbor_from_json(j.clone()).unwrap();
        json_from_cbor::<serde_json::Error>(&cbor).unwrap()
    }

    #[test]
    fn test_null_roundtrip() {
        let value = json!(null);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Null;
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_bool_roundtrip() {
        let value = json!(true);
        assert_eq!(roundtrip_json(value.clone()), value);
        let c = BaseCborValue::Bool(true);
        assert_eq!(roundtrip_cbor(c.clone()), c);

        let value = json!(false);
        assert_eq!(roundtrip_json(value.clone()), value);
        let c = BaseCborValue::Bool(false);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_zero_number() {
        let num = Number::from_f64(0f64).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_f64(-0f64).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Positive(0);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_number_positive_roundtrip() {
        let num = Number::from_u128(42).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_u128(u64::MAX as u128).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Positive(42);
        assert_eq!(roundtrip_cbor(c.clone()), c);

        let c = BaseCborValue::Positive(u64::MAX);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_number_negative_roundtrip() {
        let num = Number::from_f64(-1234.56).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_f64(
            -12342342342342325243523423432422.2112421434235798327429847298347298347298347239842,
        )
        .unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_i128(-1234).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_i128(-(u64::MAX as i128)).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_i128(-(u64::MAX as i128) + 1).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let num = Number::from_i128(-(u64::MAX as i128) + 1).unwrap();
        let value = serde_json::Value::Number(num);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Negative(123);
        assert_eq!(roundtrip_cbor(c.clone()), c);

        let c = BaseCborValue::Negative(u64::MAX);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_number_float_roundtrip() {
        let j_values = [
            serde_json::Value::Number(Number::from_f64(0.0).unwrap()),
        serde_json::Value::Number(Number::from_f64(-0.0).unwrap()),
        serde_json::Value::Number(Number::from_f64(123456.789).unwrap()),
        serde_json::Value::Number(Number::from_f64(
            12342342342342325243523423432422.2112421434235798327429847298347298347298347239842f64,
        ).unwrap())];

        let c_values = vec![
            BaseCborValue::Float(0.0),
            BaseCborValue::Float(-0.0),
            BaseCborValue::Float(123456.789),
            BaseCborValue::Float(
                12342342342342325243523423432422.2112421434235798327429847298347298347298347239842,
            ),
        ];

        for v in j_values {
            assert_eq!(roundtrip_json(v.clone()), v);
        }
        for cbor in c_values {
            assert_eq!(roundtrip_cbor(cbor.clone()), cbor);
        }
    }

    #[test]
    fn test_string_roundtrip() {
        let value = json!("hello world");
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Text("hello world".to_string());
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_array_roundtrip() {
        let value = json!([1, 2, "three", null, true]);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Array(vec![
            BaseCborValue::Positive(1),
            BaseCborValue::Text("x".into()),
        ]);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_nested_array_roundtrip() {
        let value = json!([1, [2, [3, ["four"]]]]);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Array(vec![
            BaseCborValue::Positive(1),
            BaseCborValue::Array(vec![
                BaseCborValue::Positive(1),
                BaseCborValue::Text("x".into()),
            ]),
        ]);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_object_roundtrip() {
        let value = json!({
            "a": 1,
            "b": "text",
            "c": true,
            "d": null
        });
        let result = roundtrip_json(value.clone());
        assert_eq!(result, value);

        let c = BaseCborValue::Map(vec![
            (BaseCborValue::Text("a".into()), BaseCborValue::Positive(1)),
            (
                BaseCborValue::Text("anotherField".into()),
                BaseCborValue::Text("two".into()),
            ),
        ]);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_nested_object_roundtrip() {
        let value = json!({
            "outer": {
                "inner": {
                    "value": 123
                }
            }
        });

        let result = roundtrip_json(value.clone());
        assert_eq!(result, value);

        let c = BaseCborValue::Map(vec![
            (BaseCborValue::Text("a".into()), BaseCborValue::Positive(1)),
            (
                BaseCborValue::Text("anotherField".into()),
                BaseCborValue::Map(vec![
                    (BaseCborValue::Text("a".into()), BaseCborValue::Positive(1)),
                    (
                        BaseCborValue::Text("anotherField".into()),
                        BaseCborValue::Text("two".into()),
                    ),
                ]),
            ),
        ]);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }

    #[test]
    fn test_array_of_objects_roundtrip() {
        let value = json!([
            {"a": 1},
            {"b": 2},
            {"c": [1,2,3]},
        ]);
        assert_eq!(roundtrip_json(value.clone()), value);

        let c = BaseCborValue::Array(vec![
            BaseCborValue::Positive(1),
            BaseCborValue::Map(vec![
                (BaseCborValue::Text("a".into()), BaseCborValue::Positive(1)),
                (
                    BaseCborValue::Text("anotherField".into()),
                    BaseCborValue::Text("two".into()),
                ),
            ]),
        ]);
        assert_eq!(roundtrip_cbor(c.clone()), c);
    }
}

use crate::api_types::ErrorDetail;
use crate::types::{ServerError, ValidationError};
use anyhow::{Context, Result};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::common::cbor::CborSerialize;
use concordium_rust_sdk::types::RegisteredData;
use std::collections::HashMap;

pub fn anchor_to_registered_data(
    anchor: &impl CborSerialize,
) -> Result<RegisteredData, ServerError> {
    let cbor = cbor::cbor_encode(anchor).context("cbor encode anchor")?;
    let register_data =
        RegisteredData::try_from(cbor).map_err(ServerError::AnchorPublicInfoTooBig)?;
    Ok(register_data)
}

/// Converts the provided `publicInfo` in the API requests to a HashMap of string keys
/// to Cbor values.
pub fn convert_public_info_to_hashmap_of_string_to_cbor(
    public_info: &Option<serde_json::Value>,
) -> Result<Option<HashMap<String, cbor::value::Value>>, ValidationError> {
    public_info.as_ref().map(json_to_cbor_map).transpose()
}

/// converts a json value into a Hashmap of String to cbor value. If the top level json
/// value is not an object, then return a Server error. We expect publicInfo to contain
/// a json structure of key value pairs.
pub fn json_to_cbor_map(
    value: &serde_json::Value,
) -> Result<HashMap<String, cbor::value::Value>, ValidationError> {
    match value {
        serde_json::Value::Object(json_map) => {
            let mut result = HashMap::new();
            for (key, value) in json_map {
                result.insert(key.clone(), json_to_cbor_value(value)?);
            }
            Ok(result)
        }

        other => Err(ValidationError {
            details: vec![ErrorDetail {
                code: "PUBLIC_INFO_EXPECTED_JSON".to_string(),
                path: "publicInfo".to_string(),
                message: format!("expected json at top level, got: {:?}", other),
            }],
        }),
    }
}

/// converts a json value to its corresponding Cbor Value.
/// If the json contains a number, we will do our best to try to find the corresponding
/// Cbor value type. Error if we cannot resolve the number as a valid integer or float.
fn json_to_cbor_value(value: &serde_json::Value) -> Result<cbor::value::Value, ValidationError> {
    Ok(match value {
        serde_json::Value::Null => cbor::value::Value::Null,
        serde_json::Value::Bool(b) => cbor::value::Value::Bool(*b),
        serde_json::Value::String(s) => cbor::value::Value::Text(s.clone()),

        serde_json::Value::Number(n) => {
            // With serde_json feature "arbitrary_precision", this preserves the value.
            let s = n.to_string();

            let is_integer_literal = !s.contains('.') && !s.contains('e') && !s.contains('E');

            // Check if we are dealing with an Integer literal. The supported
            // range for an integer literal storage with CBOR is shown in Cbor Value
            // Positive(u64) and Negative(u64)
            if is_integer_literal {
                const CBOR_POS_MAX: i128 = u64::MAX as i128; // 18446744073709551615
                const CBOR_NEG_MIN: i128 = -(u64::MAX as i128); // -18446744073709551615

                // Parse into i128. If it does not fit in i128, immediately its out of range
                let i: i128 = s.parse::<i128>().map_err(|_| ValidationError {
                    details: vec![ErrorDetail {
                        code: "PUBLIC_INFO_INTEGER_OUT_OF_RANGE".to_string(),
                        path: "publicInfo".to_string(),
                        message: format!(
                            "integer is too large to be represented (expected 64-bit CBOR int). Send as JSON string: {}",
                            s
                        ),
                    }],
                })?;

                // check if the parsed i128 is within the CBOR min and max
                if !(CBOR_NEG_MIN..=CBOR_POS_MAX).contains(&i) {
                    return Err(ValidationError {
                        details: vec![ErrorDetail {
                            code: "PUBLIC_INFO_INTEGER_OUT_OF_RANGE".to_string(),
                            path: "publicInfo".to_string(),
                            message: format!(
                                "Integer is outside CBOR 64-bit integer range ({}..={}): {}. Please send values outside this range as a JSON string.",
                                CBOR_NEG_MIN, CBOR_POS_MAX, s
                            ),
                        }],
                    });
                }

                // at this point we have parsed successfully as i128, and we
                // also know it lies in the valid Cbor range, so the last
                // checks here and just to determine if its positve or negative.
                if i >= 0 {
                    cbor::value::Value::Positive(i as u64)
                } else {
                    // CBOR negative argument mag = (-1 - i)
                    let mag = (-1i128 - i) as u64;
                    cbor::value::Value::Negative(mag)
                }
            } else {
                // Non integer json numbers are treated as float. Finite floats
                // are accepted, but precision may be lost with large float
                // values.
                let f = n.as_f64().ok_or_else(|| ValidationError {
                    details: vec![ErrorDetail {
                        code: "PUBLIC_INFO_NUMBER_NOT_REPRESENTABLE".to_string(),
                        path: "publicInfo".to_string(),
                        message: format!("number not representable as f64: {}", s),
                    }],
                })?;

                if !f.is_finite() {
                    return Err(ValidationError {
                        details: vec![ErrorDetail {
                            code: "PUBLIC_INFO_FLOAT_OUT_OF_RANGE".to_string(),
                            path: "publicInfo".to_string(),
                            message: format!("float out of range for f64: {}", s),
                        }],
                    });
                }

                cbor::value::Value::Float(f)
            }
        }

        serde_json::Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(json_to_cbor_value(v)?);
            }
            cbor::value::Value::Array(out)
        }

        serde_json::Value::Object(obj) => {
            let mut pairs = Vec::with_capacity(obj.len());
            for (k, v) in obj {
                pairs.push((cbor::value::Value::Text(k.clone()), json_to_cbor_value(v)?));
            }
            cbor::value::Value::Map(pairs)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Positive Test Scenarios
    #[test]
    fn test_public_info_none_returns_ok_none() {
        let public_info: Option<serde_json::Value> = None;

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);

        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_parse_public_info_success_object() {
        let session_id_val = "alpha-numeric-1234-5678-sessionid-example";
        let content_number_val = 32156789;

        let public_info = Some(serde_json::json!({
            "sessionId": session_id_val,
            "protectedContent": true,
            "contentNumber": content_number_val,
        }));

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info)
            .expect("should not error")
            .expect("should return Some(map)");

        assert_eq!(
            result.get("sessionId"),
            Some(&cbor::value::Value::Text(session_id_val.to_string()))
        );
        assert_eq!(
            result.get("protectedContent"),
            Some(&cbor::value::Value::Bool(true))
        );
        assert_eq!(
            result.get("contentNumber"),
            Some(&cbor::value::Value::Positive(32156789))
        );
    }

    #[test]
    fn test_nested_conversion() {
        let public_info = Some(serde_json::json!({
            "sessionIdRef": { "name": "Poker Stars", "category": "cards", "sub-game": 4201201, "hand": 2, "minBid": 100 },
            "tags": ["TexasHoldem", "cards101"],
            "houseWinRatio": 51,
            "negativeNumberId": -212,
        }));

        let map = convert_public_info_to_hashmap_of_string_to_cbor(&public_info)
            .unwrap()
            .unwrap();

        // get the sessionId object first
        match map.get("sessionIdRef").unwrap() {
            cbor::value::Value::Map(pairs) => {
                let name = pairs.iter().find_map(|(k, v)| match (k, v) {
                    (cbor::value::Value::Text(k), cbor::value::Value::Text(v)) if k == "name" => {
                        Some(v.clone())
                    }
                    _ => None,
                });
                assert_eq!(name.as_deref(), Some("Poker Stars"));
            }
            other => panic!("expected sessionIdRef to be CBOR Map, got {other:?}"),
        }

        // tags -> Array(["TexasHoldem","cards101"])
        assert_eq!(
            map.get("tags"),
            Some(&cbor::value::Value::Array(vec![
                cbor::value::Value::Text("TexasHoldem".to_string()),
                cbor::value::Value::Text("cards101".to_string()),
            ]))
        );

        // house win ratio should be positive 51
        assert_eq!(
            map.get("houseWinRatio"),
            Some(&cbor::value::Value::Positive(51))
        );

        // negative number id is set to -212 in json is -211 in Cbor negative
        assert_eq!(
            map.get("negativeNumberId"),
            Some(&cbor::value::Value::Negative(211))
        );
    }

    /// Error scenarios and validations.
    #[test]
    fn test_parse_public_info_fails_when_not_object() {
        let public_info = Some(serde_json::json!("hello"));

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);

        assert!(result.is_err(), "top-level non-object should error");
    }

    /// Public Info number parsing tests and constraints
    #[test]
    fn test_public_info_positive_integer_parsed_to_cbor_positive() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"simplePositiveInt": 42}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("simplePositiveInt"),
            Some(&cbor::value::Value::Positive(42))
        );
    }

    #[test]
    fn test_public_info_negative_one_parsed_to_cbor_negative_zero() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"simpleNegative": -1}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("simpleNegative"),
            Some(&cbor::value::Value::Negative(0))
        );
    }

    /// Minimum negative that can be stored in CBOR
    #[test]
    fn test_public_info_cbor_min_negative() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"i64min": -18446744073709551615}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");

        assert_eq!(
            map.get("i64min"),
            Some(&cbor::value::Value::Negative(u64::MAX - 1))
        );
    }

    /// Max positive that can be stored in CBOR
    #[test]
    fn test_public_info_cbor_max_u64_positive() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"u64max": 18446744073709551615}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("u64max"),
            Some(&cbor::value::Value::Positive(u64::MAX))
        );
    }

    /// 1 number above the Max that can be stored in CBOR positive - results
    /// in validation error for integer out of range
    #[test]
    fn test_public_info_integer_max_out_of_range() {
        // u64::MAX + 1
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"tooBig": 18446744073709551616}"#).unwrap());

        // expect error for max u64 Cbor
        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_err());

        let err = result.unwrap_err();

        assert_eq!(err.details.len(), 1);
        assert_eq!(err.details[0].code, "PUBLIC_INFO_INTEGER_OUT_OF_RANGE");
        assert_eq!(err.details[0].path, "publicInfo");
        assert!(err.details[0].message.contains("outside CBOR"));
    }

    /// 1 below the minimum that can be stored in CBOR negative.
    /// Results in Validation Error of integer out of range.
    #[test]
    fn test_public_info_integer_min_out_of_range() {
        // Minimum number supported for negative CBOR is -18446744073709551616, this is 1 below the min
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"tooBig": -18446744073709551616}"#).unwrap());

        // expect error for max u64 Cbor
        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_err());

        let err = result.unwrap_err();

        assert_eq!(err.details.len(), 1);
        assert_eq!(err.details[0].code, "PUBLIC_INFO_INTEGER_OUT_OF_RANGE");
        assert_eq!(err.details[0].path, "publicInfo");
        assert!(err.details[0].message.contains("outside CBOR"));
    }

    /// Test standard float works as expected.
    #[test]
    fn test_public_info_simple_float_parsed_to_cbor_float() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"simpleFloat": 0.1}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("simpleFloat"),
            Some(&cbor::value::Value::Float(0.1))
        );
    }

    /// Ensure float followed by .0 is stored as a Float.
    #[test]
    fn test_public_info_float_with_dot_zero_stays_float() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"onePointZero": 1.0}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("onePointZero"),
            Some(&cbor::value::Value::Float(1.0))
        );
    }

    /// Ensure scientific numbers with exponential are stored
    /// as their true Float value
    #[test]
    fn test_public_info_scientific_notation_parsed_to_cbor_float() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"sci": 1e3}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(map.get("sci"), Some(&cbor::value::Value::Float(1000.0)));
    }

    /// Test a super large Float, returns with Validation
    /// Error as not representable.
    #[test]
    fn test_public_info_float_out_of_f64_range_errors() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"tooBigFloat": 1e400}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.details.len(), 1);
        assert_eq!(err.details[0].code, "PUBLIC_INFO_NUMBER_NOT_REPRESENTABLE");
        assert_eq!(err.details[0].path, "publicInfo");
        assert!(
            err.details[0]
                .message
                .contains("number not representable as f64")
        );
    }

    // Float with 14 digits after decimal is precise
    #[test]
    fn test_public_info_float_14_digits_after_decimal_is_precise() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"preciseFloat": 99.99999999999999}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");
        assert_eq!(
            map.get("preciseFloat"),
            Some(&cbor::value::Value::Float(99.99999999999999))
        );
    }

    // Float with 15 digits after decimal loses precision
    #[test]
    fn test_public_info_float_15_digits_after_decimal_loses_precision() {
        let public_info: Option<serde_json::Value> =
            Some(serde_json::from_str(r#"{"preciseFloat": 99.999999999999999}"#).unwrap());

        let result = convert_public_info_to_hashmap_of_string_to_cbor(&public_info);
        assert!(result.is_ok());

        let map = result.unwrap().expect("should have public info");

        // proof that precision gets lost for this
        assert_eq!(
            map.get("preciseFloat"),
            Some(&cbor::value::Value::Float(100.0))
        );
    }
}

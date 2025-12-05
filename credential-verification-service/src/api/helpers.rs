use crate::{api_types::PublicInfo, types::ServerError};
use concordium_rust_sdk::common::cbor;
use std::collections::HashMap;

pub fn parse_public_info(
    info: PublicInfo,
) -> Result<HashMap<String, cbor::value::Value>, ServerError> {
    // Decode hex → bytes
    let bytes = hex::decode(info.cbor_hex)
        .map_err(|e| ServerError::InvalidPublicInfo(format!("Invalid hex: {e}")))?;

    // Decode bytes → cborValue
    let cbor_val: cbor::value::Value = cbor::cbor_decode(&bytes)
        .map_err(|e| ServerError::InvalidPublicInfo(format!("Invalid CBOR: {e}")))?;

    // Ensure it is a map
    match cbor_val {
        cbor::value::Value::Map(entries) => {
            let map: HashMap<String, cbor::value::Value> = entries
                .into_iter()
                .map(|(k, v)| match k {
                    cbor::value::Value::Text(key) => Ok((key, v)),
                    _ => Err(ServerError::InvalidPublicInfo(
                        "Non-text map key in CBOR".to_string(),
                    )),
                })
                .collect::<Result<_, _>>()?;

            Ok(map)
        }
        _ => Err(ServerError::InvalidPublicInfo(
            "Public info must decode to a CBOR map".to_string(),
        )),
    }
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use crate::{api::helpers::parse_public_info, api_types::PublicInfo};
    use concordium_rust_sdk::common::cbor::{self, value::Value as CborValue};

    #[test]
    fn test_parse_public_info() {
        let cbor_value = CborValue::Map(vec![
            (CborValue::Text("a".into()), CborValue::Positive(1)),
            (
                CborValue::Text("anotherField".into()),
                CborValue::Text("two".into()),
            ),
        ]);

        let expected: HashMap<String, CborValue> = HashMap::from([
            ("a".to_string(), CborValue::Positive(1)),
            (
                "anotherField".to_string(),
                CborValue::Text("two".to_string()),
            ),
        ]);

        let cbor_vec = cbor::cbor_encode(&cbor_value).unwrap();
        let public_info = PublicInfo {
            cbor_hex: hex::encode(cbor_vec),
        };
        let parsed = parse_public_info(public_info).unwrap();
        assert_eq!(parsed, expected);
    }
}

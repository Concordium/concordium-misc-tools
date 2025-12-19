use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::common::cbor::CborSerialize;
use concordium_rust_sdk::types::RegisteredData;

pub fn anchor_to_registered_data(anchor: &impl CborSerialize) -> anyhow::Result<RegisteredData> {
    let cbor = cbor::cbor_encode(anchor)?;
    let register_data = RegisteredData::try_from(cbor)?;
    Ok(register_data)
}

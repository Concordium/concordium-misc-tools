use crate::types::ServerError;
use anyhow::Context;
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::common::cbor::CborSerialize;
use concordium_rust_sdk::types::RegisteredData;

pub fn anchor_to_registered_data(
    anchor: &impl CborSerialize,
) -> Result<RegisteredData, ServerError> {
    let cbor = cbor::cbor_encode(anchor).context("cbor encode anchor")?;
    let register_data = RegisteredData::try_from(cbor).map_err(ServerError::PublicInfoTooBig)?;
    Ok(register_data)
}

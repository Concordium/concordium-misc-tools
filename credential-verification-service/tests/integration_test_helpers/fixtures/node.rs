use crate::integration_test_helpers::fixtures;
use crate::integration_test_helpers::node_stub::NodeStub;
use concordium_rust_sdk::id::constants::ArCurve;
use concordium_rust_sdk::id::types::GlobalContext;

pub fn stub_common(node_stub: &NodeStub, global_context: &GlobalContext<ArCurve>) {
    node_stub.mock(|when, then| {
        when.path("/concordium.v2.Queries/GetBlockInfo");
        then.pb(fixtures::chain::block_info())
            .headers([("blockhash", hex::encode(fixtures::chain::BLOCK_HASH))]);
    });

    node_stub.mock(|when, then| {
        when.path("/concordium.v2.Queries/GetCryptographicParameters");
        then.pb(fixtures::chain::cryptographic_parameters(&global_context))
            .headers([("blockhash", hex::encode(fixtures::chain::BLOCK_HASH))]);
    });
}

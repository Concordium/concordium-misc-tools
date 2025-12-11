use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    IdentityCredentialType, IdentityProviderDid, Nonce, RequestedIdentitySubjectClaimsBuilder,
    RequestedStatement, VerificationRequest,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::id::id_proof_types::AttributeInSetStatement;
use concordium_rust_sdk::id::types::{AttributeTag, IpIdentity};
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::web3id::Web3IdAttribute;
use concordium_rust_sdk::web3id::did::Network;
use credential_verification_service::api_types::CreateVerificationRequest;
use reqwest::StatusCode;

#[tokio::test]
async fn test_create_verification_request() {
    let handle = server::start_server();

    let identity_claims = RequestedIdentitySubjectClaimsBuilder::new()
        .source(IdentityCredentialType::IdentityCredential)
        .issuer(IdentityProviderDid {
            network: Network::Testnet,
            identity_provider: IpIdentity(1),
        })
        .statement(RequestedStatement::AttributeInSet(
            AttributeInSetStatement {
                attribute_tag: AttributeTag(1),
                set: [Web3IdAttribute::String("val1".parse().unwrap())]
                    .into_iter()
                    .collect(),
                _phantom: Default::default(),
            },
        ))
        .build();

    let create_request = CreateVerificationRequest {
        nonce: Nonce([1u8; 32]),
        connection_id: "conid1".to_string(),
        resource_id: "resid1".to_string(),
        context_string: "contextstr".to_string(),
        requested_claims: vec![identity_claims.into()],
        public_info: Some(
            [(
                "key1".to_string(),
                cbor::value::Value::Text("value1".to_string()),
            )]
            .into_iter()
            .collect(),
        ),
    };

    let txn_hash = fixtures::generate_txn_hash();
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/SendBlockItem");
        then.pb(generated::TransactionHash::from(&txn_hash));
    });

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verification_request: VerificationRequest = resp.json().await.unwrap();
    assert_eq!(verification_request.anchor_transaction_hash, txn_hash);
}

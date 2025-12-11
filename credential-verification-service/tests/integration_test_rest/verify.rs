use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::base::web3id::v1::anchor::{IdentityCredentialType, IdentityProviderDid, RequestedIdentitySubjectClaimsBuilder, RequestedStatement, VerificationRequest};
use concordium_rust_sdk::id::id_proof_types::AttributeInSetStatement;
use concordium_rust_sdk::id::types::{AttributeTag, IpIdentity};
use concordium_rust_sdk::v2::generated;
use concordium_rust_sdk::web3id::did::Network;
use concordium_rust_sdk::web3id::Web3IdAttribute;
use credential_verification_service::api_types::{VerifyPresentationRequest, VerifyPresentationResponse};
use reqwest::StatusCode;

fn verification_request() -> VerificationRequest {
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

    VerificationRequest {
        context: Default::default(),
        subject_claims: vec![identity_claims.into()],
        anchor_transaction_hash: fixtures::generate_txn_hash(),
    }
}

/// Test verify identity based presentation
#[tokio::test]
async fn test_verify_identity_based() {
    let handle = server::start_server();

    let verification_request = verification_request();

    let presentation = todo!();

    let verify_request = VerifyPresentationRequest {
        audit_record_id: "auditrecid1".to_string(),
        public_info: Some(
            fixtures::public_info()
        ),
        presentation,
        verification_request,
    };

    let txn_hash = fixtures::generate_txn_hash();
    handle.node_stub().mock(|when, then| {
        when.path("/concordium.v2.Queries/SendBlockItem");
        then.pb(generated::TransactionHash::from(&txn_hash));
    });

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&verify_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verify_response: VerifyPresentationResponse = resp.json().await.unwrap();

}

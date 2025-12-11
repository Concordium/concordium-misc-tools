use crate::integration_test_helpers::fixtures::credentials::{
    AccountCredentialsFixture, IdentityCredentialsFixture, seed0,
};

use concordium_rust_sdk::base::hashes::TransactionHash;
use concordium_rust_sdk::base::web3id::v1::anchor::{
    IdentityCredentialType, IdentityProviderDid, Nonce, RequestedIdentitySubjectClaimsBuilder,
    RequestedStatement, VerifiablePresentationRequestV1, VerifiablePresentationV1,
    VerificationRequest,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::id::id_proof_types::AttributeInSetStatement;
use concordium_rust_sdk::id::types::{AttributeTag, IpIdentity};
use concordium_rust_sdk::web3id::Web3IdAttribute;
use concordium_rust_sdk::web3id::did::Network;
use credential_verification_service::api_types::CreateVerificationRequest;

use std::collections::HashMap;

mod credentials;

pub fn generate_txn_hash() -> TransactionHash {
    TransactionHash::new(rand::random())
}

pub fn public_info() -> HashMap<String, cbor::value::Value> {
    [(
        "key1".to_string(),
        cbor::value::Value::Text("value1".to_string()),
    )]
    .into_iter()
    .collect()
}

pub fn generate_presentation_identity(
    id_cred: &IdentityCredentialsFixture,
    request: VerifiablePresentationRequestV1,
) -> VerifiablePresentationV1 {
    let now = chrono::Utc::now();
    let presentation = request
        .prove_with_rng(
            &credentials::global_context(),
            [id_cred.private_inputs()].into_iter(),
            &mut seed0(),
            now,
        )
        .expect("prove");

    presentation
}

pub fn generate_presentation_account(
    account_cred: &AccountCredentialsFixture,
    request: VerifiablePresentationRequestV1,
) -> VerifiablePresentationV1 {
    let now = chrono::Utc::now();
    let presentation = request
        .prove_with_rng(
            &credentials::global_context(),
            [account_cred.private_inputs()].into_iter(),
            &mut seed0(),
            now,
        )
        .expect("prove");

    presentation
}

pub fn verification_request() -> VerificationRequest {
    let identity_claims = RequestedIdentitySubjectClaimsBuilder::new()
        .source(IdentityCredentialType::IdentityCredential)
        .source(IdentityCredentialType::AccountCredential)
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
        anchor_transaction_hash: generate_txn_hash(),
    }
}

pub fn create_verification_request() -> CreateVerificationRequest {
    let identity_claims = RequestedIdentitySubjectClaimsBuilder::new()
        .source(IdentityCredentialType::IdentityCredential)
        .source(IdentityCredentialType::AccountCredential)
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

    CreateVerificationRequest {
        nonce: Nonce([1u8; 32]),
        connection_id: "conid1".to_string(),
        resource_id: "resid1".to_string(),
        context_string: "contextstr".to_string(),
        requested_claims: vec![identity_claims.into()],
        public_info: Some(public_info()),
    }
}

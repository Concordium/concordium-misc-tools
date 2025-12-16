use crate::integration_test_helpers::fixtures::credentials::{
    AccountCredentialsFixture, IdentityCredentialsFixture, seed0,
};

use concordium_rust_sdk::base::hashes::{BlockHash, TransactionHash};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    ContextLabel, IdentityCredentialType, IdentityProviderDid, LabeledContextProperty, Nonce,
    RequestedIdentitySubjectClaimsBuilder, RequestedStatement, RequestedSubjectClaims,
    UnfilledContextInformation, UnfilledContextInformationBuilder, VerifiablePresentationRequestV1,
    VerifiablePresentationV1, VerificationRequest, VerificationRequestAnchor,
    VerificationRequestData,
};
use concordium_rust_sdk::common::cbor;
use concordium_rust_sdk::id::id_proof_types::{AttributeInSetStatement, AttributeValueStatement};
use concordium_rust_sdk::id::types::{AttributeTag, GlobalContext, IpIdentity};
use concordium_rust_sdk::web3id::Web3IdAttribute;
use concordium_rust_sdk::web3id::did::Network;
use credential_verification_service::api_types::{
    CreateVerificationRequest, VerifyPresentationRequest,
};

use crate::integration_test_helpers::fixtures;
use concordium_rust_sdk::base::web3id::v1::{
    AccountBasedSubjectClaims, AtomicStatementV1, ContextInformation, IdentityBasedSubjectClaims,
    SubjectClaims,
};
use concordium_rust_sdk::common;
use concordium_rust_sdk::id::constants::{ArCurve, AttributeKind};
use std::collections::{BTreeMap, HashMap};
use std::fmt::Debug;
use std::marker::PhantomData;
use std::str::FromStr;

pub mod chain;
pub mod credentials;

pub fn public_info() -> HashMap<String, cbor::value::Value> {
    [(
        "key1".to_string(),
        cbor::value::Value::Text("value1".to_string()),
    )]
    .into_iter()
    .collect()
}

pub fn generate_presentation_identity(
    global_context: &GlobalContext<ArCurve>,
    id_cred: &IdentityCredentialsFixture,
    request: VerifiablePresentationRequestV1,
) -> VerifiablePresentationV1 {
    let now = chrono::Utc::now();
    let presentation = request
        .prove_with_rng(
            global_context,
            [id_cred.private_inputs()].into_iter(),
            &mut seed0(),
            now,
        )
        .expect("prove");

    presentation
}

pub fn generate_presentation_account(
    global_context: &GlobalContext<ArCurve>,
    account_cred: &AccountCredentialsFixture,
    request: VerifiablePresentationRequestV1,
) -> VerifiablePresentationV1 {
    let now = chrono::Utc::now();
    let presentation = request
        .prove_with_rng(
            global_context,
            [account_cred.private_inputs()].into_iter(),
            &mut seed0(),
            now,
        )
        .expect("prove");

    presentation
}

pub fn verification_request(anchor_transaction_hash: TransactionHash) -> VerificationRequest {
    let statements = statements_and_attributes().0;

    let identity_claims = RequestedIdentitySubjectClaimsBuilder::new()
        .source(IdentityCredentialType::IdentityCredential)
        .source(IdentityCredentialType::AccountCredential)
        .issuer(IdentityProviderDid {
            network: Network::Testnet,
            identity_provider: IpIdentity(1),
        })
        .issuer(IdentityProviderDid {
            network: Network::Testnet,
            identity_provider: IpIdentity(2),
        })
        .statements(statements)
        .build();

    VerificationRequest {
        context: UnfilledContextInformationBuilder::new()
            .given(LabeledContextProperty::Nonce(Nonce(rand::random())))
            .given(LabeledContextProperty::ConnectionId("conid1".to_string()))
            .given(LabeledContextProperty::ResourceId("resid1".to_string()))
            .given(LabeledContextProperty::ContextString(
                "contextstr".to_string(),
            ))
            .requested(ContextLabel::BlockHash)
            .build(),
        subject_claims: vec![identity_claims.into()],
        anchor_transaction_hash,
    }
}

/// Statements and attributes that make the statements true
fn statements_and_attributes<TagType: FromStr + common::Serialize + Ord>() -> (
    Vec<RequestedStatement<TagType>>,
    BTreeMap<TagType, Web3IdAttribute>,
)
where
    <TagType as FromStr>::Err: Debug,
{
    let statements = vec![RequestedStatement::AttributeInSet(
        AttributeInSetStatement {
            attribute_tag: AttributeTag(1).to_string().parse().unwrap(),
            set: [
                Web3IdAttribute::String(AttributeKind::try_new("ff".into()).unwrap()),
                Web3IdAttribute::String(AttributeKind::try_new("aa".into()).unwrap()),
                Web3IdAttribute::String(AttributeKind::try_new("zz".into()).unwrap()),
            ]
            .into_iter()
            .collect(),
            _phantom: PhantomData,
        },
    )];

    let attributes = [(
        AttributeTag(1).to_string().parse().unwrap(),
        Web3IdAttribute::String(AttributeKind::try_new("aa".into()).unwrap()),
    )]
    .into_iter()
    .collect();

    (statements, attributes)
}

pub fn verification_request_to_verifiable_presentation_request_identity(
    id_cred: &IdentityCredentialsFixture,
    verification_request: &VerificationRequest,
) -> VerifiablePresentationRequestV1 {
    VerifiablePresentationRequestV1 {
        context: unfilled_context_information_to_context_information(&verification_request.context),
        subject_claims: verification_request
            .subject_claims
            .iter()
            .map(|claims| requested_subject_claims_to_subject_claims_identity(id_cred, claims))
            .collect(),
    }
}

pub fn verification_request_to_verifiable_presentation_request_account(
    account_cred: &AccountCredentialsFixture,
    verification_request: &VerificationRequest,
) -> VerifiablePresentationRequestV1 {
    VerifiablePresentationRequestV1 {
        context: unfilled_context_information_to_context_information(&verification_request.context),
        subject_claims: verification_request
            .subject_claims
            .iter()
            .map(|claims| requested_subject_claims_to_subject_claims_account(account_cred, claims))
            .collect(),
    }
}

fn unfilled_context_information_to_context_information(
    context: &UnfilledContextInformation,
) -> ContextInformation {
    ContextInformation {
        given: context
            .given
            .iter()
            .map(|prop| prop.to_context_property())
            .collect(),
        requested: context
            .requested
            .iter()
            .map(|label| match label {
                ContextLabel::BlockHash => LabeledContextProperty::BlockHash(BlockHash::from(
                    fixtures::chain::GENESIS_BLOCK_HASH,
                )),
                _ => panic!("unexpected label"),
            })
            .map(|prop| prop.to_context_property())
            .collect(),
    }
}

fn requested_subject_claims_to_subject_claims_identity(
    id_cred: &IdentityCredentialsFixture,
    claims: &RequestedSubjectClaims,
) -> SubjectClaims<ArCurve, Web3IdAttribute> {
    match claims {
        RequestedSubjectClaims::Identity(claims) => {
            let statements = claims
                .statements
                .iter()
                .map(requested_statement_to_statement)
                .collect();

            SubjectClaims::Identity(IdentityBasedSubjectClaims {
                network: Network::Testnet,
                issuer: id_cred.issuer,
                statements,
            })
        }
    }
}

fn requested_subject_claims_to_subject_claims_account(
    account_cred: &AccountCredentialsFixture,
    claims: &RequestedSubjectClaims,
) -> SubjectClaims<ArCurve, Web3IdAttribute> {
    match claims {
        RequestedSubjectClaims::Identity(id_claims) => {
            let statements = id_claims
                .statements
                .iter()
                .map(requested_statement_to_statement)
                .collect();

            SubjectClaims::Account(AccountBasedSubjectClaims {
                network: Network::Testnet,
                issuer: account_cred.issuer,
                cred_id: account_cred.cred_id,
                statements,
            })
        }
    }
}

fn requested_statement_to_statement(
    statement: &RequestedStatement<AttributeTag>,
) -> AtomicStatementV1<ArCurve, AttributeTag, Web3IdAttribute> {
    match statement {
        RequestedStatement::RevealAttribute(stmt) => {
            AtomicStatementV1::AttributeValue(AttributeValueStatement {
                attribute_tag: stmt.attribute_tag,
                attribute_value: Web3IdAttribute::String(
                    AttributeKind::try_new("testvalue".into()).unwrap(),
                ),
                _phantom: Default::default(),
            })
        }
        RequestedStatement::AttributeInRange(stmt) => {
            AtomicStatementV1::AttributeInRange(stmt.clone())
        }
        RequestedStatement::AttributeInSet(stmt) => AtomicStatementV1::AttributeInSet(stmt.clone()),
        RequestedStatement::AttributeNotInSet(stmt) => {
            AtomicStatementV1::AttributeNotInSet(stmt.clone())
        }
    }
}

pub fn create_verification_request() -> CreateVerificationRequest {
    let statements = statements_and_attributes().0;

    let identity_claims = RequestedIdentitySubjectClaimsBuilder::new()
        .source(IdentityCredentialType::IdentityCredential)
        .source(IdentityCredentialType::AccountCredential)
        .issuer(IdentityProviderDid {
            network: Network::Testnet,
            identity_provider: IpIdentity(1),
        })
        .statements(statements)
        .build();

    CreateVerificationRequest {
        connection_id: "conid1".to_string(),
        resource_id: "resid1".to_string(),
        context_string: None,
        requested_claims: vec![identity_claims.into()],
        public_info: Some(public_info()),
    }
}

#[derive(Debug, Clone)]
pub struct VerifyPresentationRequestFixture {
    pub request: VerifyPresentationRequest,
    pub anchor: VerificationRequestAnchor,
    pub anchor_txn_hash: TransactionHash,
}

pub fn verify_request_account(
    global_context: &GlobalContext<ArCurve>,
    account_cred: &AccountCredentialsFixture,
) -> VerifyPresentationRequestFixture {
    let anchor_txn_hash = chain::generate_txn_hash();
    let verification_request = verification_request(anchor_txn_hash);

    let verifiable_presentation_request =
        verification_request_to_verifiable_presentation_request_account(
            account_cred,
            &verification_request,
        );
    let presentation = generate_presentation_account(
        global_context,
        account_cred,
        verifiable_presentation_request,
    );

    let verification_data = VerificationRequestData {
        context: verification_request.context.clone(),
        subject_claims: verification_request.subject_claims.clone(),
    };

    let request = VerifyPresentationRequest {
        audit_record_id: "auditrecid1".to_string(),
        public_info: Some(public_info()),
        presentation,
        verification_request,
    };

    let anchor = verification_data.to_anchor(Some(public_info()));

    VerifyPresentationRequestFixture {
        anchor_txn_hash,
        request,
        anchor,
    }
}

pub fn verify_request_identity(
    global_context: &GlobalContext<ArCurve>,
    id_cred: &IdentityCredentialsFixture,
) -> VerifyPresentationRequestFixture {
    let anchor_txn_hash = chain::generate_txn_hash();
    let verification_request = verification_request(anchor_txn_hash);

    let verifiable_presentation_request =
        verification_request_to_verifiable_presentation_request_identity(
            id_cred,
            &verification_request,
        );
    let presentation =
        generate_presentation_identity(global_context, id_cred, verifiable_presentation_request);

    let verification_data = VerificationRequestData {
        context: verification_request.context.clone(),
        subject_claims: verification_request.subject_claims.clone(),
    };

    let request = VerifyPresentationRequest {
        audit_record_id: "auditrecid1".to_string(),
        public_info: Some(public_info()),
        presentation,
        verification_request,
    };

    let anchor = verification_data.to_anchor(Some(public_info()));

    VerifyPresentationRequestFixture {
        anchor_txn_hash,
        request,
        anchor,
    }
}

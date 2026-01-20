use crate::integration_test_helpers::{fixtures::{self, ATTRIBUTE_TAG_COUNTRY_OF_RESIDENCE, make_country_set_statement}, server};
use concordium_rust_sdk::{base::web3id::v1::anchor::{
    ContextLabel, IdentityCredentialType, IdentityProviderDid, RequestedIdentitySubjectClaims, RequestedIdentitySubjectClaimsBuilder, RequestedStatement, RequestedSubjectClaims, UnfilledContextInformation, VerificationRequest
}, id::{constants::AttributeKind, id_proof_types::{AttributeInRangeStatement, AttributeInSetStatement}, types::IpIdentity}, web3id::{Web3IdAttribute, did::Network}};
use credential_verification_service::api_types::{ErrorDetail, ErrorResponse};
use reqwest::StatusCode;
use std::{collections::{BTreeSet, HashSet}, marker::PhantomData};

/// Test create verification request
#[tokio::test]
async fn test_create_verification_request() {
    let handle = server::start_server();

    let create_request = fixtures::create_verification_request();

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verification_request: VerificationRequest = resp.json().await.unwrap();
    handle
        .node_client_stub()
        .expect_send_block_item(&verification_request.anchor_transaction_hash);
    assert_eq!(
        verification_request.subject_claims,
        create_request.requested_claims
    );
    let expected_requested_labels: HashSet<_> = [ContextLabel::BlockHash].into_iter().collect();
    assert_eq!(
        get_requested_property_labels(&verification_request.context),
        expected_requested_labels
    );
    let expected_given_labels: HashSet<_> = [
        ContextLabel::ResourceId,
        ContextLabel::Nonce,
        ContextLabel::ConnectionId,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        get_given_property_labels(&verification_request.context),
        expected_given_labels
    );
    assert_eq!(
        get_given_property_value(&verification_request.context, ContextLabel::ResourceId)
            .expect("resource id property"),
        create_request.resource_id
    );
    assert!(
        get_given_property_value(&verification_request.context, ContextLabel::ContextString)
            .is_none()
    );
    assert_eq!(
        get_given_property_value(&verification_request.context, ContextLabel::ConnectionId)
            .expect("connection id property"),
        create_request.connection_id
    );
    get_given_property_value(&verification_request.context, ContextLabel::Nonce)
        .expect("nonce property");
}

/// Test create verification request. Test specifying ContextString property in verification context
#[tokio::test]
async fn test_create_verification_request_with_context_string() {
    let handle = server::start_server();

    let mut create_request = fixtures::create_verification_request();
    create_request.context_string = Some("contextstring1".to_string());

    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let verification_request: VerificationRequest = resp.json().await.unwrap();
    let expected_given_labels: HashSet<_> = [
        ContextLabel::ResourceId,
        ContextLabel::Nonce,
        ContextLabel::ContextString,
        ContextLabel::ConnectionId,
    ]
    .into_iter()
    .collect();
    assert_eq!(
        get_given_property_labels(&verification_request.context),
        expected_given_labels
    );
    assert_eq!(
        get_given_property_value(&verification_request.context, ContextLabel::ContextString)
            .expect("context string property"),
        "contextstring1"
    );
}


// ----------------------------------
// Error Response Structure Scenarios
// ----------------------------------
#[tokio::test]
async fn test_create_verification_request_attribute_in_range_bound_not_numeric() {
    let handle = server::start_server();

    // create the verification request api payload
    // modify with range not numeric
    let mut create_verification_request = fixtures::create_verification_request();

    // create invalid attribute in range statement
    let attribute_in_range_statement = RequestedStatement::AttributeInRange(
        AttributeInRangeStatement {
            attribute_tag: fixtures::ATTRIBUTE_TAG_DOB,
            lower: Web3IdAttribute::String(AttributeKind::try_new("abcdef".into()).unwrap()),
            upper: Web3IdAttribute::String(AttributeKind::try_new("20240101".into()).unwrap()),
            _phantom: PhantomData,
        }
    );

    // modify create verification request now with the invalid statement
    create_verification_request.requested_claims = vec![RequestedSubjectClaims::Identity(
        RequestedIdentitySubjectClaims { 
            statements: vec![attribute_in_range_statement], 
            issuers: vec![IdentityProviderDid { identity_provider: IpIdentity(0u32), network: Network::Testnet}], 
            source: vec![IdentityCredentialType::IdentityCredential]
        }
    )];

    // Call the API with the invalid request
    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_verification_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Assertions and expected validation codes and messages
    let expected_code = "VALIDATION_ERROR";
    let expected_message = "Validation errors have occurred. Please check the details below for more information.";
    let expected_detail_code = "ATTRIBUTE_IN_RANGE_STATEMENT_NOT_NUMERIC";
    let expected_detail_message = "Attribute in range statement, is a numeric range check between a lower and upper bound. These must be numeric values.";

    assert_eq!(expected_code, error_response.error.code);
    assert_eq!(expected_message, error_response.error.message);
    assert_eq!(false, error_response.error.retryable);
    assert_eq!("dummy", error_response.error.trace_id);

    assert!(error_response.error.details.len() == 1);
    let detail = &error_response.error.details[0];
    assert_eq!(detail.code, expected_detail_code);
    assert_eq!(detail.message, expected_detail_message);
}


#[tokio::test]
async fn test_create_verification_request_attribute_in_range_upper_should_be_greater_than_lower() {
    let handle = server::start_server();

    // create the verification request api payload
    // modify with range not numeric
    let mut create_verification_request = fixtures::create_verification_request();

    // create invalid attribute in range statement
    let attribute_in_range_statement = RequestedStatement::AttributeInRange(
        AttributeInRangeStatement {
            attribute_tag: fixtures::ATTRIBUTE_TAG_DOB,
            lower: Web3IdAttribute::String(AttributeKind::try_new("20200101".into()).unwrap()),
            upper: Web3IdAttribute::String(AttributeKind::try_new("19990101".into()).unwrap()),
            _phantom: PhantomData,
        }
    );

    // modify create verification request now with the invalid statement
    create_verification_request.requested_claims = vec![RequestedSubjectClaims::Identity(
        RequestedIdentitySubjectClaims { 
            statements: vec![attribute_in_range_statement], 
            issuers: vec![IdentityProviderDid { identity_provider: IpIdentity(0u32), network: Network::Testnet}], 
            source: vec![IdentityCredentialType::IdentityCredential]
        }
    )];

    // Call the API with the invalid request
    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_verification_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();

    // Assertions and expected validation codes and messages
    let expected_code = "VALIDATION_ERROR";
    let expected_message = "Validation errors have occurred. Please check the details below for more information.";
    let expected_detail_code = "ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID";
    let expected_detail_message = "Provided `upper bound: 19990101` must be greater than `lower bound: 20200101`.";

    assert_eq!(expected_code, error_response.error.code);
    assert_eq!(expected_message, error_response.error.message);
    assert_eq!(false, error_response.error.retryable);
    assert_eq!("dummy", error_response.error.trace_id);

    assert!(error_response.error.details.len() == 1);
    let detail = &error_response.error.details[0];
    assert_eq!(detail.code, expected_detail_code);
    assert_eq!(detail.message, expected_detail_message);
}


#[tokio::test]
async fn test_create_verification_request_multiple_errors_range_and_set() {
    let handle = server::start_server();

    // create the verification request api payload
    // modify with range not numeric
    let mut create_verification_request = fixtures::create_verification_request();

    // invalid range statement for dob - upper is less than lower
    let attribute_in_range_statement = RequestedStatement::AttributeInRange(
        fixtures::make_range_statement("19900101".into(), "19890101".into())
    );

    // invalid set statement for country of residence, UK is not valid it should be GB (Great Britain).
    let set_statement = RequestedStatement::AttributeInSet(
        fixtures::make_country_set_statement(vec!["UK"])
    );

    // modify create verification request now with the invalid statement
    create_verification_request.requested_claims = vec![RequestedSubjectClaims::Identity(
        RequestedIdentitySubjectClaims { 
            statements: vec![attribute_in_range_statement, set_statement], 
            issuers: vec![IdentityProviderDid { identity_provider: IpIdentity(0u32), network: Network::Testnet}], 
            source: vec![IdentityCredentialType::IdentityCredential]
        }
    )];

    // Call the API with the invalid request
    let resp = handle
        .rest_client()
        .post("verifiable-presentations/create-verification-request")
        .json(&create_verification_request)
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    // unwrap the Error Response
    let error_response: ErrorResponse = resp.json().await.unwrap();
    assert_eq!(error_response.error.details.len(), 2);

    // Assertions and expected validation codes and messages
    let expected_code = "VALIDATION_ERROR";
    let expected_message = "Validation errors have occurred. Please check the details below for more information.";
    assert_eq!(&error_response.error.code, expected_code);
    assert_eq!(&error_response.error.message, expected_message);

    assert_has_detail(
        &error_response.error.details,
        "ATTRIBUTE_IN_RANGE_STATEMENT_BOUNDS_INVALID",
        "Provided `upper bound: 19890101` must be greater than `lower bound: 19900101`.",
    );

    assert_has_detail(
        &error_response.error.details,
        "COUNTRY_CODE_INVALID",
        "Country code must be 2 letter and both uppercase following the ISO3166-1 alpha-2 uppercase standard. (e.g `DE`)",
    );
}


fn get_given_property_value(
    context: &UnfilledContextInformation,
    label: ContextLabel,
) -> Option<String> {
    for prop in &context.given {
        if prop.label() == label {
            return Some(prop.value().to_string());
        }
    }
    None
}

fn get_requested_property_labels(context: &UnfilledContextInformation) -> HashSet<ContextLabel> {
    context.requested.iter().copied().collect()
}

fn get_given_property_labels(context: &UnfilledContextInformation) -> HashSet<ContextLabel> {
    context.given.iter().map(|prop| prop.label()).collect()
}

fn assert_has_detail(details: &[ErrorDetail], code: &str, message: &str) {
    assert!(
        details.iter().any(|d| d.code == code && d.message == message),
        "missing expected detail: code={code}, message={message}\nactual details: {:#?}",
        details
    );
}
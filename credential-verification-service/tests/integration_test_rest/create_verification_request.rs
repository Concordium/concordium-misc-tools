use crate::integration_test_helpers::{fixtures, server};
use concordium_rust_sdk::base::web3id::v1::anchor::{
    ContextLabel, UnfilledContextInformation, VerificationRequest,
};
use reqwest::StatusCode;
use std::collections::HashSet;

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
        get_given_property_value(&verification_request.context, ContextLabel::ResourceId)
            .expect("resource id property"),
        create_request.resource_id
    );
    assert_eq!(
        get_given_property_value(&verification_request.context, ContextLabel::ContextString)
            .expect("context string property"),
        create_request.context_string
    );
    assert_eq!(
        get_given_property_value(&verification_request.context, ContextLabel::ConnectionId)
            .expect("connection id property"),
        create_request.connection_id
    );
    get_given_property_value(&verification_request.context, ContextLabel::Nonce)
        .expect("nonce property");
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

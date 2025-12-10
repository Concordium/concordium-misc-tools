//! Handler for create-verification-request endpoint.
use crate::{
    api_types::{ClaimType, CreateVerificationRequest, SubjectClaims},
    types::{ServerError, Service},
};
use axum::{Json, extract::State};
use concordium_rust_sdk::{
    base::web3id::v1::{AtomicStatementV1, anchor::{
        IdentityProviderDid, LabeledContextProperty, RequestedIdentitySubjectClaims, RequestedStatement, RequestedSubjectClaims, UnfilledContextInformation, UnfilledContextInformationBuilder, VerificationRequest, VerificationRequestData
    }}, common::types::TransactionTime, id::{constants::{ArCurve, AttributeKind}, id_proof_types::{self, RevealAttributeStatement, Statement}, types::{AttributeTag, IpIdentity}}, v2::{QueryError, RPCError}, web3id::{Web3IdAttribute, did::Network, v1::{
        AnchorTransactionMetadata, CreateAnchorError::Query,
        create_verification_request_and_submit_request_anchor,
    }}
};
use std::sync::Arc;

/// Handler for the create verification request API
pub async fn create_verification_request(
    State(state): State<Arc<Service>>,
    Json(params): Json<CreateVerificationRequest>,
) -> Result<Json<VerificationRequest>, ServerError> {
    let context = UnfilledContextInformationBuilder::new_simple(
        params.nonce,
        params.connection_id,
        params.context_string,
    )
    .given(LabeledContextProperty::ResourceId(params.rescource_id))
    .build();

    // build the verification request data from our API
    let verification_request_data = build_verification_request_data(
        context, 
        params.requested_claims, 
        state.network
    );


    // Transaction should expiry after some seconds.
    let expiry = TransactionTime::seconds_after(state.transaction_expiry_secs);

    let mut node_client = state.node_client.clone();

    // Get the current nonce for the backend wallet and lock it. This is necessary
    // since it is possible that API requests come in parallel. The nonce is
    // increased by 1 and its lock is released after the transaction is submitted to
    // the blockchain.
    let mut account_sequence_number = state.nonce.lock().await;

    let anchor_transaction_metadata = AnchorTransactionMetadata {
        signer: &state.account_keys,
        sender: state.account_keys.address,
        account_sequence_number: *account_sequence_number,
        expiry,
    };

    let verification_request = create_verification_request_and_submit_request_anchor(
        &mut node_client,
        anchor_transaction_metadata,
        verification_request_data.clone(),
        None,
    )
    .await;

    match verification_request {
        Ok(req) => {
            // If the submission of the anchor transaction was successful,
            // increase the account_sequence_number tracked in this service.
            *account_sequence_number = account_sequence_number.next();
            Ok(Json(req))
        }

        Err(e) => {
            // If the error is due to an account sequence number mismatch,
            // refresh the value in the state and try to resubmit the transaction.
            if let Query(QueryError::RPCError(RPCError::CallError(ref err))) = e {
                let msg = err.message();
                let is_nonce_err = msg == "Duplicate nonce" || msg == "Nonce too large";

                if is_nonce_err {
                    tracing::warn!(
                        "Unable to submit transaction on-chain successfully due to account nonce mismatch: {}.
                        Account nonce will be re-freshed and transaction will be re-submitted.",
                        msg
                    );

                    // Refresh nonce
                    let nonce_response = node_client
                        .get_next_account_sequence_number(&state.account_keys.address)
                        .await
                        .map_err(|e| ServerError::SubmitAnchorTransaction(e.into()))?;

                    *account_sequence_number = nonce_response.nonce;

                    tracing::info!("Refreshed account nonce successfully.");

                    // Retry anchor transaction.
                    let meta = AnchorTransactionMetadata {
                        signer: &state.account_keys,
                        sender: state.account_keys.address,
                        account_sequence_number: nonce_response.nonce,
                        expiry,
                    };

                    let verification_request =
                        create_verification_request_and_submit_request_anchor(
                            &mut node_client,
                            meta,
                            verification_request_data,
                            None,
                        )
                        .await?;

                    tracing::info!(
                        "Successfully submitted anchor transaction after the account nonce was refreshed."
                    );

                    *account_sequence_number = account_sequence_number.next();

                    return Ok(Json(verification_request));
                }
            }

            Err(ServerError::SubmitAnchorTransaction(e))
        }
    }
}


/// Converts the API's high level abstraction of subject claims into the
/// RequestedSubjectClaims which are made up of the Atomic statements for the 
/// VerificationRequestData.
/// Finally, returns the VerificationRequestData to the caller.
fn build_verification_request_data(
    context: UnfilledContextInformation,
    subject_claims: Vec<SubjectClaims>,
    network: Network
) -> VerificationRequestData {
    let requested_statements = vec![];
    let requested_subject_claims_list: Vec<RequestedSubjectClaims> = vec![];

    // deal with root subject claims array
    for subject_claims in subject_claims {

        // for each subject claim, go through the claims
        let statement = Statement::new();
        for claim in subject_claims.claims {
            map_claim_type(claim, statement);
        }

        // now convert statement to requested statement
        for atomic_statement in statement.statements {
            let requested_statement = statement_to_requested_statement(atomic_statement);
            requested_statements.push(requested_statement);
        }

        let issuers = vec![];
        for issuer in subject_claims.issuers {
            let idp = IdentityProviderDid {
                network: network,
                identity_provider: IpIdentity(u32::from(issuer))
            };
            issuers.push(idp);
        }

        let requested_subject_claims = RequestedIdentitySubjectClaims {
            issuers: issuers,
            source: subject_claims.source,
            statements: requested_statements
        };

        let rsc = RequestedSubjectClaims::Identity(requested_subject_claims);

        requested_subject_claims_list.push(rsc);
    }

    VerificationRequestData {
        context,
        subject_claims: requested_subject_claims_list
    }
}

/// Mapping function to map from a provided claim type to append the atomic statement to the statement
/// passed as argument
/// let statement = Statement::new();
fn map_claim_type(
    claim_type: ClaimType,
    statement: Statement<ArCurve, AttributeKind>
) -> Option<id_proof_types::Statement<ArCurve, AttributeKind>> {
    match claim_type {
        ClaimType::AgeOlderThan { min_age } => {
            statement.older_than(min_age)
        }
        ClaimType::AgeYoungerThan { max_age } => {
            statement.younger_than(max_age)
        }
        ClaimType::AgeInRange { min_age, max_age } => {
            statement.age_in_range(min_age, max_age)
        }
        ClaimType::NationalityInSet { set } => {
            statement.nationality_in(set)
        }
        ClaimType::NationalityNotInSet { set } => {
            statement.nationality_not_in(set)
        }
        ClaimType::ResidentInSet { set } => {
            statement.residence_in(set)
        }
        ClaimType::ResidentNotInSet { set } => {
            statement.residence_not_in(set)
        }
    }
}

/// converts an atomic statement into a RequestedStatement which is used to build the VerificationRequestData
fn statement_to_requested_statement(
    statement: &AtomicStatementV1<ArCurve, AttributeTag, Web3IdAttribute>,
) -> RequestedStatement<AttributeTag> {
    match statement {
        AtomicStatementV1::AttributeValue(stmt) => {
            RequestedStatement::RevealAttribute(RevealAttributeStatement {
                attribute_tag: stmt.attribute_tag,
            })
        }
        AtomicStatementV1::AttributeInRange(stmt) => {
            RequestedStatement::AttributeInRange(stmt.clone())
        }
        AtomicStatementV1::AttributeInSet(stmt) => RequestedStatement::AttributeInSet(stmt.clone()),
        AtomicStatementV1::AttributeNotInSet(stmt) => {
            RequestedStatement::AttributeNotInSet(stmt.clone())
        }
    }
}
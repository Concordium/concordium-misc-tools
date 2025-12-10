use std::collections::BTreeSet;

use concordium_rust_sdk::{
    base::{
        hashes::TransactionHash,
        web3id::v1::{
            PresentationV1,
            anchor::{self, IdentityCredentialType, VerificationAuditRecord, VerificationRequest},
        },
    },
    id::constants::{ArCurve, AttributeKind, IpPairing},
    web3id::Web3IdAttribute,
};

/// Parameters posted to this service when calling the API
/// endpoint `/verifiable-presentations/create-verification-request`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct CreateVerificationRequest {
    /// The nonce included in the verification request context.  
    /// This nonce must be freshly and randomly generated for each request so that the
    /// verification request cannot be inferred from the on-chain request anchor hash
    /// by attempting to guess its preimage.  
    /// In short: to keep the anchor hash random, this nonce must be truly random.
    pub nonce: anchor::Nonce,
    /// An identifier for some connection (e.g. wallet-connect topic) included in the verification request context.
    pub connection_id: String,
    /// A rescource id to track the connected website (e.g. website URL or TLS fingerprint).
    pub rescource_id: String,
    /// A general purpose string value included in the verification request context.
    pub context_string: String,
    /// The subject claims being requested to be proven.
    pub requested_claims: Vec<SubjectClaims>,
    // TODO: Remaining missing field
    // Additional public info which will be included in the anchor transaction (VRA)
    // that is submitted on-chain.
    // pub public_info: HashMap<String, SerdeCborValue>,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct SubjectClaims {
    pub claim_type: String,
    pub source: Vec<IdentityCredentialType>,
    pub issuers: Vec<u8>,
    pub claims: Vec<ClaimType>
}

/// Represents a high level claim to be made about a given subject.
/// This will later be translated to the Atomic Statements that make up Credential Statements
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum ClaimType {
    AgeOlderThan { min_age: u64 },
    AgeYoungerThan { max_age: u64 },
    AgeInRange { min_age: u64, max_age: u64 },
    ResidentInSet{ set: BTreeSet<AttributeKind> },
    ResidentNotInSet{ set: BTreeSet<AttributeKind> },
    NationalityInSet{ set: BTreeSet<AttributeKind> },
    NationalityNotInSet{set: BTreeSet<AttributeKind> },
}

/// API request payload for verifying a presentation
/// endpoint: `/verifiable-presentations/verify`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct VerifyPresentationRequest {
    /// Audit record id that the client wants to include in the audit anchor.
    pub audit_record_id: String,
    /// Verifiable presentation that contains verifiable credentials each
    /// consisting of subject claims and proofs of them.
    /// It is the response to proving a [`RequestV1`] with [`RequestV1::prove`].
    pub presentation: PresentationV1<IpPairing, ArCurve, Web3IdAttribute>,
    /// A verification request that specifies which subject claims are requested from a credential holder
    /// and in which context.
    pub verification_request: VerificationRequest,
}

/// Response to verifying a presentation
/// endpoint: `/verifiable-presentations/verify`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct VerifyPresentationResponse {
    /// Whether the verification was successful or not for the presentation
    pub result: VerificationResult,
    /// Audit record which contains the complete verified request and presentation
    pub verification_audit_record: VerificationAuditRecord,
    /// Audit anchor transaction hash reference that was put on chain
    pub anchor_transaction_hash: Option<TransactionHash>,
}

/// Presentation Verification Result, contains: Success or Failed with a String message
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub enum VerificationResult {
    /// Verified
    Verified,
    /// Failed with reason for the verification failing
    Failed(String),
}

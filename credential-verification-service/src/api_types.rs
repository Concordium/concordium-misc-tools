use concordium_rust_sdk::{
    base::web3id::v1::{
        PresentationV1,
        anchor::{self, RequestedSubjectClaims, VerificationRequest},
    },
    id::constants::{ArCurve, IpPairing},
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
    pub requested_claims: Vec<RequestedSubjectClaims>,
    // TODO: Remaining missing field
    // Additional public info which will be included in the anchor transaction (VRA)
    // that is submitted on-chain.
    // pub public_info: HashMap<String, SerdeCborValue>,
}

/// API request payload for verifying a presentation
/// endpoint: `/verifiable-presentations/verify`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
pub struct VerifyPresentationRequest {
    /// Verifiable presentation that contains verifiable credentials each
    /// consisting of subject claims and proofs of them.
    /// It is the response to proving a [`RequestV1`] with [`RequestV1::prove`].
    pub presentation: PresentationV1<IpPairing, ArCurve, Web3IdAttribute>,
    /// A verification request that specifies which subject claims are requested from a credential holder
    /// and in which context.
    pub verification_request: VerificationRequest,
}

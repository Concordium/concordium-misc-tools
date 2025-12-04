use concordium_rust_sdk::base::web3id::v1::anchor::{self, RequestedSubjectClaims};

/// Parameters posted to this service when calling the API
/// endpoint `/verifiable-presentations/create-verification-request`.
#[derive(serde::Deserialize, serde::Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
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
    pub subject_claims: Vec<RequestedSubjectClaims>,
    // TODO: Remaining missing field
    // Additional public info which will be included in the anchor transaction (VRA)
    // that is submitted on-chain.
    // pub public_info: HashMap<String, SerdeCborValue>,
}

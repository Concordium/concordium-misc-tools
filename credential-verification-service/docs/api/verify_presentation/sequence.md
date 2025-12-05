
# Verify Presentation – POST /verifiable-presentations/verify

```mermaid
sequenceDiagram
    participant Merchant
    participant CredentialVerificationService
    participant RustSDK
    participant ConcordiumBase
    participant GRPCNode

    Merchant->>CredentialVerificationService: POST /verifiable-presentations/verify {VerifyPresentationRequest}
    CredentialVerificationService->>RustSDK:  web3id::v1::verify_presentation_and_submit_audit_anchor

    RustSDK-->GRPCNode: get_cryptographic_parameters
    GRPCNode-->RustSDK: endpoints::QueryResult<QueryResponse<types::CryptographicParameters>>

    RustSDK-->GRPCNode: lookup_request_anchor
    GRPCNode-->RustSDK: VerificationRequestAnchorAndBlockHash

    RustSDK->>GRPCNode: getBlockInfo
    GRPCNode->>RustSDK: BlockInfo 
    RustSDK->>GRPCNode: lookup_verification_materials_and_validity
    GRPCNode->>RustSDK: Vec<VerificationMaterialWithValidity>
    RustSDK->>ConcordiumBase: anchor::verify_presentation_with_request_anchor
    ConcordiumBase->>RustSDK: PresentationVerificationResult

    RustSDK->>RustSDK: VerificationAuditRecord 
    RustSDK->>GRPCNode: submit verification audit record anchor (register data TX)
    GRPCNode->>RustSDK: Transaction Hash

    RustSDK->>CredentialVerificationService: VerifyPresentationResponse(verification result, audit record, audit anchor hash)
    CredentialVerificationService->>Merchant: VerifyPresentationResponse
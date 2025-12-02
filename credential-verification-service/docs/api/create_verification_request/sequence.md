
# Create Verification Request – POST /verifiable-presentations/create-verification-request

## Request Payload

```mermaid
sequenceDiagram
    participant Merchant
    participant CredentialVerificationService
    participant RustSDK
    participant ConcordiumBase
    participant GRPCNode

    Merchant->>CredentialVerificationService: POST /verifiable-presentations/create-verification-request {CreateVerificationRequest}
    CredentialVerificationService->>CredentialVerificationService: build VerificationRequestData
    CredentialVerificationService->>RustSDK: anchor::create_verification_request_and_submit_request_anchor
    
    RustSDK->>ConcordiumBase: Create TX for VRA (send::register_data)
    ConcordiumBase-->>RustSDK: AccountTransaction<EncodedPayload>
    
    RustSDK->>GRPCNode: Send block item (client::send_block_item) (Account TX)
    GRPCNode-->>RustSDK: TransactionHash
    
    RustSDK->>RustSDK: build VerificationRequest
    RustSDK-->>CredentialVerificationService: Response VerificationRequest
    CredentialVerificationService-->>Merchant: 200 OK {VerificationRequest}

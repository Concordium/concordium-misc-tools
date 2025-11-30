
# Create Verification Request – POST /verifiable-presentations/verify

## Request Payload

```mermaid
sequenceDiagram
    participant Merchant
    participant CredentialVerificationService
    participant RustSDK
    participant ConcordiumBase
    participant GRPCNode

    Merchant->>CredentialVerificationService: POST /verifiable-presentations/verify {VerifyPresentationRequest}
    <TODO>
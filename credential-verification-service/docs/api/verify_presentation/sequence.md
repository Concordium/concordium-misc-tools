
# Create Verification Request – POST /verifiable-presentations/verify

## Request Payload

<TODO - remaining sequence below to be filled in>

```mermaid
sequenceDiagram
    participant Merchant
    participant CredentialVerificationService
    participant RustSDK
    participant ConcordiumBase
    participant GRPCNode

    Merchant->>CredentialVerificationService: POST /verifiable-presentations/verify {VerifyPresentationRequest}
Architecture Overivew

```mermaid
flowchart LR
    subgraph ClientSide["Merchant Client"]
        MUI["Merchant App / Backend"]
    end

    subgraph VerificationService["Credential Verification Service (Rust)"]
        API["REST API / HTTP Endpoint"]
        SDK["Rust SDK Integration\n(anchor + tx building)"]
    end

    subgraph Concordium["Concordium Network"]
        Node["gRPC Concordium Node"]
    end

    MUI -->|HTTP POST /verifiable-presentations/create-verification-request| API
    API --> SDK
    SDK -->|gRPC| Node
    Node -->|TransactionHash| SDK
    SDK --> API
    API -->|HTTP 200 VerificationRequest| MUI
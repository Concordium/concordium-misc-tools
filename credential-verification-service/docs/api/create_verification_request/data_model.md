# Create Verification Request – Data Model

This document defines the data model for the `Verify Presentation` API.

CreateVerificationRequest is the request sent by the merchant
the response that the merchant receives is the VerificationRequest that can 
later be used in the prove and verify flow


---

# Request Payload

The request sent by the Merchant when initiating a verification flow.

## Structure (Mermaid)

```mermaid
classDiagram 

    %% Request to the API to create a verification request
    class CreateVerificationRequest {
        <<Request>>
        string nonce
        string connectionId
        string resourceId
        string contextString
        HashMap publicInfo
        SubjectClaim[] claims
    }

    class SubjectClaim {
        int[] trustedIDPs
        IdentityCredentialType credentialType
        int[] issuers
        SubjectStatement[] statements
    }

    class SubjectStatement{
        ProvingStatementType type,
        string tag,
        int lower_bound,
        int upper_bound,
        Set set
    }

    class ProvingStatementType {
        <<enum>>
        AttributeInRange
        AttributeInSet
        AttributeNotInSet
        RevealAttribute
    }

    class AttributeInRange{
        string tag
        int lower_bound
        int upper_bound
    }

    class AttributeInSet{
        string tag
        Set set
    }

    CreateVerificationRequest --> SubjectClaim: claims
    SubjectClaim --> SubjectStatement
    SubjectClaim --> IdentityCredentialType
    SubjectStatement --> ProvingStatementType: type
    SubjectStatement --> AttributeInRange
    SubjectStatement --> AttributeInSet


    %% middle layer conversion into the Verification Request Data from the Merchants request above
    class VerificationRequestData {
        context: UnfilledContextInformation
        subject_claims: RequestedSubjectClaims[]
    }

    VerificationRequestData --> UnfilledContextInformation: context
    VerificationRequestData --> RequestedSubjectClaims: subject_claims

    %%Response to the API to create request for Verification of credentials
    class VerificationRequest {
        <<Response>>
        UnfilledContextInformation context
        RequestedSubjectClaims[] subject_claims
        TransactionHash anchor_transaction_hash
    }

    class UnfilledContextInformation {
        GivenContext[] given
        ContextLabel[] requested
    }

    class GivenContext {
        <<enum>>
        Nonce
        PaymentHash
        BlockHash
        ConnectionId
        ResourceId
        ContextString
    }

    class ContextLabel {
        <<enum>>
        Nonce
        PaymentHash
        BlockHash
        ConnectionId
        ResourceId
        ContextString
    }

        %% Claims
    class RequestedSubjectClaims {
        <<enum>>
        Identity
    }

    class RequestedIdentitySubjectClaims {
        Statement statements
        IdentityProviderDid[] issuers
        IdentityCredentialType[] source
    }

    %% Statements
    class Statement {
        AtomicStatement[] statements
    }

    class AtomicStatement {
        <<enum>>
        RevealAttribute
        AttributeInRange
        AttributeInSet
        AttributeNotInSet
    }

    class IdentityCredentialType{
        <<enum>>
        IdentityCredential
        AccountCredential
    }

    class IdentityProviderDid{
        network: Network,
        identity_provider: IpIdentity,
    }

    class Network{
        <<enum>>
        Testnet
        Mainnet
    }

    class IpIdentity {
        <<u32>>
    }

    %% Relationships
    VerificationRequest --> UnfilledContextInformation: context
    VerificationRequest --> RequestedSubjectClaims: subject_claims
    UnfilledContextInformation --> GivenContext: given
    UnfilledContextInformation --> ContextLabel: requested
    RequestedSubjectClaims --> RequestedIdentitySubjectClaims: identity
    RequestedIdentitySubjectClaims --> Statement: statements
    RequestedIdentitySubjectClaims --> IdentityProviderDid: issuers
    RequestedIdentitySubjectClaims --> IdentityCredentialType: source
    Statement --> AtomicStatement
    IdentityProviderDid --> Network: network
    IdentityProviderDid --> IpIdentity: identity_provider
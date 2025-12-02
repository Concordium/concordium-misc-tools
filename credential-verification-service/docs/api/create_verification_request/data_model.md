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
        string connectionId
        string description
        string resourceId
        ClaimType claimType
        int[] trustedIDPs
        VerificationCheck[] verificationChecks
    }

    class ClaimType {
        <<enum>>
        Identity
    }

    class VerificationCheck {
        <<enum>>
        AtLeastAge(int) // age in years
        NationalityInRegion(Region)
    }

    class Region {
        <<enum>>
        EU
        AFRICA
        AMERICAS
        APAC
    }

    CreateVerificationRequest --> ClaimType: claimType
    CreateVerificationRequest --> VerificationCheck: verification_checks
    VerificationCheck --> Region

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
        IdentityProviderMethod[] issuers
        CredentialType[] source
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

    %% Relationships
    VerificationRequest --> UnfilledContextInformation: context
    VerificationRequest --> RequestedSubjectClaims: subject_claims
    UnfilledContextInformation --> GivenContext: given
    UnfilledContextInformation --> ContextLabel: requested
    RequestedSubjectClaims --> RequestedIdentitySubjectClaims: identity
    RequestedIdentitySubjectClaims --> Statement: statements
    RequestedIdentitySubjectClaims --> IdentityProviderMethod: issuers
    Statement --> AtomicStatement

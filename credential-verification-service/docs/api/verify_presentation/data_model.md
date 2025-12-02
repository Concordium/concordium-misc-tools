# Create Verification Request – Data Model

This document defines the data model for the `Create Verification Request` API.

CreateVerificationRequest is the request sent by the merchant
the response that the merchant receives is the VerificationRequest that can 
later be used in the prove and verify flow


---

# Request and Response

The request sent by the Merchant when initiating a verification flow.

## Structure (Mermaid)

```mermaid
classDiagram 

    %% Request to the API to verify a presentation
    class VerifyPresentationRequest {
        <<API Request>>
        presentation: PresentationV1,
        verificationRequest: VerificationRequest [Verification request model](../create_verification_request/data_model.md)
    }

    class PresentationV1 {
        presentation_context: ContextInformation
        verifiable_credentials: CredentialV1[]
        pub linking_proof: LinkingProofV1,
    }

    class ContextInformation {
        given: ContextProperty[]
        requested: ContextProperty[]
    }

    class CredentialV1 {
        <<enum>>
        Account
        Identity
    }

    class AccountBasedCredentialV1 {
        issuer: IpIdentity
        subject: AccountCredentialSubject
        proof: ConcordiumZkProof
    }

    class IdentityBasedCredentialV1{
        issuer: IpIdentity
        validity: CredentialValidity
        subject: IdentityCredentialSubject
        proof: ConcordiumZkProof
    }

    class ContextProperty {
        label: String,
        context: String
    }



    %%%% Response Structure
    class PresentationVerificationData{
        <<API Response>>
        verification_result: PresentationVerificationResult
        audit_record: VerificationAuditRecord
        anchor_transaction_hash: TransactionHash
    }

    class PresentationVerificationResult{
        <<enum>>
        Verified
        Failed(CredentialInvalidReason)
    }

    class VerificationAuditRecord{
        version: u16,
        id: String,
        request: VerificationRequest,
        presentation: VerifiablePresentationV1,
    }

    class AccountCredentialSubject{
        network: Network
        cred_id: CredentialRegistrationId
        statements: AtomicStatementV1[]
    }

    class Network {
        <<enum>>
        Testnet
        Mainnet
    }

    class AtomicStatement {
        <<enum>>
        RevealAttribute
        AttributeInRange
        AttributeInSet
        AttributeNotInSet
    }

    class IdentityCredentialSubject {
        network: Network
        cred_id: IdentityCredentialEphemeralId(vec u8)
        statements: AtomicStatementV1[]
    }

    VerifyPresentationRequest --> PresentationV1: presentation
    VerifyPresentationRequest --> VerificationRequest: verificationRequest
    PresentationV1 --> ContextInformation
    PresentationV1 --> CredentialV1
    CredentialV1 --> AccountBasedCredentialV1
    CredentialV1 --> IdentityBasedCredentialV1
    ContextInformation --> ContextProperty

    AccountBasedCredentialV1 --> AccountCredentialSubject
    AccountCredentialSubject --> Network
    AccountCredentialSubject --> AtomicStatement

    IdentityBasedCredentialV1 --> IdentityCredentialSubject
    IdentityCredentialSubject --> Network
    IdentityBasedCredentialV1 --> AtomicStatement


    PresentationVerificationData --> PresentationVerificationResult: verification_result
    PresentationVerificationData --> VerificationAuditRecord: audit_record
    VerificationAuditRecord --> PresentationV1
    VerificationAuditRecord --> VerificationRequest
## Unreleased

- Initial service which implements verifying Concordium V1 verifiable presentations and creating verification 
  requests. This includes submitting the verification request anchors (VRA), verifying the request anchor and
  submitting the verification audit anchor (VAA). The following endpoints exist in the initial version
  - `/verifiable-presentations/verify`
  - `/verifiable-presentations/create-verification-request`

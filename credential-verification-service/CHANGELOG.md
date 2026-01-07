## Unreleased

## 0.2.1

- Change to wait for the anchor transaction to be finalized when verifying a proof.

## 0.2.0

- Refactored transaction submit logic
- Changed a number of errors resulting in HTTP status code 500 to now result in status code in 4xx range
- Changed verification failure results to include a failure code in addition to a failure message
 
## 0.1.0

- Initial service which implements verifying Concordium V1 verifiable presentations and creating verification
  requests. This includes submitting the verification request anchors (VRA), verifying the request anchor and
  submitting the verification audit anchor (VAA). The following endpoints exist in the initial version
  - `/verifiable-presentations/verify`
  - `/verifiable-presentations/create-verification-request`

# Credential Verification Service

This service is used for carrying out operations related to requesting verification for credentials and verifying Presentations.

## Build Docker image

`docker build -f Dockerfile ../ -t credential-verification-service`

## Run Docker image

The following is the template command you need to use to run the service locally with docker. 

Note: The `CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT` environment variable below does not need to be modified, this is the path that the key is expected to exist inside the container. 

```
docker run --rm \
  -e CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT="http://grpc.testnet.concordium.com:20000" \
  -e CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS="0.0.0.0:8000" \
  -e CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS="0.0.0.0:8001" \
  -e LOG_LEVEL="info" \
  -v /path/to/wallet_key.export:/keys/test_key.export:ro \
  -e CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT="/keys/test_key.export" \
  -p 8000:8000 \
  -p 8001:8001 \
  credential-verification-service
```


you should then be able to curl the health endpoint from outside the container, for example:

`curl http://localhost:8001/health`



## API Documentation

### Create Verification Request

Endpoint: HTTP POST /verifiable-presentations/create-verification-request {CreateVerificationRequest}

Purpose: Creates a Verification Request in order to prove some statements about the credentials.

Process Overview:
- Submits a register data transaction to the concordium network, in the form of a VRA (Verifiable Request Anchor)
- Returns the `VerificationRequest` which contains the anchor transaction hash

Diagrams and Sample Payloads: 
- [Sequence Diagram](docs/api/create_verification_request/sequence.md)
- [Data Model (Request + Response)](docs/api/create_verification_request/data_model.md)
- [Example Payloads](docs/api/create_verification_request/examples.md)

### Verify Presentation

Endpoint: HTTP POST /verifiable-presentations/verify {VerifiablePresentationRequest}

Purpose: To verify a Presentation

Process Overview:
- Submits a register data transaction to the concordium network, in the form of a VAA (Verifiable Audit Anchor)
- Returns the `AnchoredVerificationAuditRecordResponse` which contains the Audit record and the VAA transaction hash

Diagrams and Sample Payloads: 
- [Sequence Diagram](docs/api/verify_presentation/sequence.md)
- [Data Model (Request + Response)](docs/api/verify_presentation§/data_model.md)
- [Example Payloads](docs/api/verify_presentation//examples.md)


## Architecture
- 🗺️ [Architecture Overview](docs/architecture.md)


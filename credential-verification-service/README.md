# Credential Verification Service

This service is used for carrying out operations related to requesting verification for credentials and verifying Presentations.

## Build Docker image

`docker build -f Dockerfile ../ -t concordium/credential-verification-service:0.1.0`

## Pull Docker image from Docker Hub 

```
docker pull concordium/credential-verification-service:0.1.0
```

## Run Docker image

The following is the template command you need to use to run the service locally with docker. 

Note: The `CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT` environment variable below does not need to be modified, this is the path that the key is expected to exist inside the container. 

```
docker run --rm \
  -e CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT="https://grpc.testnet.concordium.com:20000" \
  -e CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS="0.0.0.0:8000" \
  -e CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS="0.0.0.0:8001" \
  -e LOG_LEVEL="info" \
  -v /path/to/wallet_key.export:/keys/test_key.export:ro \
  -e CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT="/keys/test_key.export" \
  -p 8000:8000 \
  -p 8001:8001 \
  concordium/credential-verification-service:0.1.0
```

You should then be able to curl the health endpoint from outside the container, for example:

`curl http://localhost:8001/health`

## Build the service from the source code

Make sure to check out git submodules

```console
git submodule update --init --recursive
```

You can build the service locally as follows:

```
cargo build
```

This will produce a single binary `../target/debug/credential-verification-service`.

## Run the service from the source code

You can run the service locally as follows:

```
cargo run -- --node-endpoint https://grpc.testnet.concordium.com:20000 --account 4bbdAUCDK2D6cUvUeprGr4FaSaHXKuYmYVjyCa4bXSCu3NUXzA.export
```

## Configuration options

The following options are supported:

- `--node-endpoint [env: CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT]`: the URL of the node's GRPC V2 interface, e.g., http://node.testnet.concordium.com:20000
- `--request-timeout [env: CREDENTIAL_VERIFICATION_SERVICE_REQUEST_TIMEOUT]`: The request timeout for a request to be processed with the credential service api in milliseconds (defaults to 15 seconds if not given).
- `--grpc-node-request-timeout [env: CREDENTIAL_VERIFICATION_GRPC_NODE_REQUEST_TIMEOUT]`: The request timeout to the Concordium node in milliseconds (defaults to 1 second if not given).
- `--log-level [env: CREDENTIAL_VERIFICATION_SERVICE_LOG_LEVEL]`: The log level (defaults to info if not given).
- `--account [env: CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT]`: The path to the account key file.
- `--api-address [env: CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS]`: The socket address where the service exposes its API (defaults to `127.0.0.1:8000` if not given).
- `--monitoring-address [env: CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS]`: The socket address used for health and metrics monitoring (defaults to `127.0.0.1:8001` if not given).
- `--transaction-expiry [env: CREDENTIAL_VERIFICATION_SERVICE_TRANSACTION_EXPIRY]`: The number of seconds in the future when the anchor transactions should expiry (defaults to 15 seconds if not given).
- `--anchor-wait-for-finalization-timeout [env: CREDENTIAL_VERIFICATION_ANCHOR_WAIT_FOR_FINALIZATION_TIMEOUT]`: Timeout in milliseconds when waiting for the anchor transaction to finalize when verifying the proof (defaults to 6 seconds if not given).
- `--acquire-account-sequence-lock-timeout [env: CREDENTIAL_VERIFICATION_ACQUIRE_ACCOUNT_SEQUENCE_LOCK_TIMEOUT]`: Timeout in milliseconds for holding the lock on the account sequence number (defaults to 5 seconds if not given).

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
- Returns the `PresentationVerificationData` response which contains the verification result, the audit anchor record and the audit anchor transaction hash.

Diagrams and Sample Payloads: 
- [Sequence Diagram](docs/api/verify_presentation/sequence.md)
- [Data Model (Request + Response)](docs/api/verify_presentation/data_model.md)
- [Example Payloads](docs/api/verify_presentation//examples.md)


## Architecture
- 🗺️ [Architecture Overview](docs/architecture.md)

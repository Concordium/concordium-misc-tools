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

You should then be able to curl the health endpoint from outside the container, for example:

`curl http://localhost:8001/health`

## Build the service from the source code

You can build the serive locally as follows:

```
cargo build
```

## Run the servie from the source code

You can run the serive locally as follows:

```
cargo run -- --node-endpoint https://grpc.testnet.concordium.com:20000 --account 4bbdAUCDK2D6cUvUeprGr4FaSaHXKuYmYVjyCa4bXSCu3NUXzA.export
```

## Configuration options

The following options are supported:

- `--node_endpoint [env: CREDENTIAL_VERIFICATION_SERVICE_NODE_GRPC_ENDPOINT]`: the URL of the node's GRPC V2 interface, e.g., http://node.testnet.concordium.com:20000
- `--request_timeout [env: REQUEST_TIMEOUT]`:` The request timeout (both of request to the node and server requests) in milliseconds.
- `--log-level [env: LOG_LEVEL]`: The maximum log level` (defaults to info if not given).
- `--account [env: CREDENTIAL_VERIFICATION_SERVICE_ACCOUNT]`: The path to the account key 
- `--api_address [env: CREDENTIAL_VERIFICATION_SERVICE_API_ADDRESS]`: The socket address where the service exposes its API.
- `--monitoring_address [env: CREDENTIAL_VERIFICATION_SERVICE_MONITORING_ADDRESS]`: The socket address used for health and metrics monitoring.


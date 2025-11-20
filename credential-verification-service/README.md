# Credential Verification Service

This service is used for carrying out operations related to requesting verification for credentials and verifying Presentations.

## Build Docker image

`docker build -f Dockerfile ../ -t credential-verification-service`

## Run Docker image

The following is the template command you need to use to run the service locally with docker. 

Note: The `Account` environment variable below does not need to be modified, this is the path that the key is expected to exist inside the container. 

```
EXAMPLE:

docker run --rm \
  -e NODE_GRPC_ENDPOINT="http://grpc.testnet.concordium.com:20000" \
  -e API_ADDRESS="0.0.0.0:8000" \
  -e MONTITORING_ADDRESS="0.0.0.0:8001" \
  -e LOG_LEVEL="info" \
  -e ACCOUNT="/keys/test_key.export" \
  -v /Users/robertsquire/projects/concordium/test_key.export:/keys/test_key.export:ro \
  -p 8000:8000 \
  -p 8001:8001 \
  credential-verification-service
```

you should then be able to curl the health endpoint from outside the container, for example:

`curl http://localhost:8001/health`

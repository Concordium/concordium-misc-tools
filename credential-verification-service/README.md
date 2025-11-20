# Credential Verification Service




## Health endpoint

`curl http://127.0.01:8000/health`


## Build Docker image

`docker build -f Dockerfile ../ -t credential-verification-service`

## Run Docker image

The following is the template command you need to use to run the service locally with docker. 

Note: The `Account` environment variable below does not need to be modified, this is the path that the key is expected to exist inside the container. 

```
TEMPLATE: 

docker run --rm \
  -e CREDENTIAL_VERIFICATION_NODE_GRPC_ENDPOINT="http://grpc.testnet.concordium.com:20000" \
  -e CREDENTIAL_VERIFICATION_ADDRESS="<HOST>:<PORT>" \
  -e LOG_LEVEL="info" \
  -e ACCOUNT="/keys/test_key.export" \
  -v <ACCOUNT_KEY_FILE_PATH_LOCALLY>:/keys/test_key.export:ro \
  -p 8000:8000 \
  -d \
  credential-verification-service

EXAMPLE:

docker run --rm \
  -e CREDENTIAL_VERIFICATION_NODE_GRPC_ENDPOINT="http://grpc.testnet.concordium.com:20000" \
  -e CREDENTIAL_VERIFICATION_ADDRESS="0.0.0.0:8000" \
  -e LOG_LEVEL="info" \
  -e ACCOUNT="/keys/test_key.export" \
  -v /Users/robertsquire/projects/concordium/test_key.export:/keys/test_key.export:ro \
  -p 8000:8000 \
  -d \
  credential-verification-service
```

you should then be able to curl the health endpoint from outside the container, for example:

`curl http://localhost:8000/health`

## network dashboard proxy

This proxy supports the block explorer in the [network
dashboard](https://github.com/Concordium/concordium-network-dashboard).

The API exposed here is what is needed by the network dashboard, and that is how
this project is going to be developed. There are no guarantees on API stability.

The proxy exposes the following endpoints
- GET `v1/consensusStatus`
- GET `v1/blockSummary/:blockHash`
- GET `v1/blockInfo/:blockHash`
- GET `v1/blocksByHeight/:height`
- GET `v1/transactionStatus/:transactionHash`

The responses will be either in the 200 range, or 404 if the object (e.g.,
transaction) with the given identifier is not found, or 500 if there are
problems with communication with the node. If the request cannot be parsed
(e.g., the transaction hash is not parseable) then 400 is returned.

## Build

To build run `cargo build --release`. This produces the binary `target/release/network-dashboard-proxy`.

## Docker image

A docker image containing the relayer and API server can be built using the
provided [`Dockerfile`](./scripts/build.Dockerfile) as follows **from the root
of the repository**. Make sure to do a full repository checkout first using

```
git submodule update --init --recursive
```

Then run

```
docker build \
    --build-arg build_image=rust:1.67-buster\
    --build-arg base_image=debian:buster\
    -f network-dashboard-proxy/scripts/build.Dockerfile\
    -t network-dashboard-proxy:latest .
```

## Run

The proxy needs access to the node and supports the following environment
variables

- `NETWORK_DASHBOARD_PROXY_CONCORDIUM_NODE` (defaults to http://localhost:20000)
  the address of the Concordium node. If the address starts with `https` then a
  TLS connection to the node will be established.
- `NETWORK_DASHBOARD_PROXY_API_LISTEN_ADDRESS` (defaults to 0.0.0.0:8080) the
  address where the server will listen for incoming connections.
- `NETWORK_DASHBOARD_PROXY_LOG_LEVEL` (defaults to `info`), the maximum log
  level.
- `NETWORK_DASHBOARD_PROXY_LOG_HEADERS` (defaults to `false`), whether to
  include request and response headers in logs. This is useful for debugging,
  but should be off in production.
- `NETWORK_DASHBOARD_PROXY_REQUEST_TIMEOUT` (defaults to 5000ms), the maximum
  request processing time. This includes the timeout of the node request.

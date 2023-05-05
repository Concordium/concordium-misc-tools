## chain prometheus exporter

A Prometheus exporter for monitoring the chain. Currently it provides balances
of specific accounts.

The intention of this tool is to serve as an exporter of relevant **chain**
data, which can be used for setting alerts.

## Build

To build run `cargo build --release`. This produces the binary `target/release/chain-prometheus-exporter`.

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
    -f chain-prometheus-exporter/scripts/build.Dockerfile\
    -t chain-prometheus-exporter:latest .
```

The image has the binary `chain-prometheus-exporter` installed in
`/usr/local/bin`.

## Run

The proxy needs access to the node and supports the following environment
variables

- `CHAIN_PROMETHEUS_EXPORTER_CONCORDIUM_NODE` (defaults to http://localhost:20000)
  the address of the Concordium node. If the address starts with `https` then a
  TLS connection to the node will be established.
- `CHAIN_PROMETHEUS_EXPORTER_API_LISTEN_ADDRESS` (defaults to 0.0.0.0:8080) the
  address where the server will listen for incoming connections.
- `CHAIN_PROMETHEUS_EXPORTER_ACCOUNTS` the comma-separated list of strings in
  the form label:address where `label` is a valid prometheus label, and
  `address` is an account address.


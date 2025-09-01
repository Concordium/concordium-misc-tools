# The ID verifier test tool

This page describes the id verifier backend that is used for integration testing
of ID 2.0 functionality in wallets. **This is not how a production id verifier
is meant to work.**

# Supported configuration options

The following environment variables (command line options) are supported
- `ENDPOINT` (`--node`) the URL of the node's GRPC V2 interface, e.g., http://localhost:20000
- `PORT` (`--port`) the port on which the server will listen for incoming requests.
- `LOG_LEVEL` (`--log-level`) maximum log level (defaults to `debug` if not given)
- `STATIC_DIR` (`--dir`) if set, serve files from the given directory under the `/` path.

All of the above is available by using `--help` to get usage information.

The verifier is a simple server that exposes two endpoints `POST /inject` and
`POST /prove`, and optionally serves files under `GET /`. 
The latter is intended for hosting the frontend for simplicity.

The overall flow is that a statement is **injected** into the server, which
responds with a challenge. Then `prove` endpoint can be called with a proof of
the statement, and the challenge that was used. The challenge is used to match
the proof to the statement to be proved.

All of the server state is kept in memory and thus does not survive a restart.
There is also no purging.

See [src/main.rs](./src/main.rs) for the formats of requests and responses. Both
requests and responses are JSON encoded. The `/prove` endpoint responds with
status `200 OK` if the proof is acceptable, and with invalid request otherwise.
The requests are handled by `handle_inject_statement` and `handle_provide_proof`
handlers in [src/main.rs](./src/main.rs). See there for the format of the
request.

The server needs access to the node so that it can get the requested credential
from the node during proof validation.

# Contributing

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.0-4baaaa.svg)](https://github.com/Concordium/.github/blob/main/.github/CODE_OF_CONDUCT.md)

This repository's CI automatically checks formatting and common problems in rust.
Changes to any of the packages must be such that
- ```cargo clippy --all``` produces no warnings
- ```cargo fmt``` makes no changes.

Everything in this repository should build with stable rust at the moment (at least version 1.56 and up).
(the exact version used by the CI can be found in [.github/workflows/ci.yaml](https://github.com/Concordium/concordium-misc-tools/blob/main/.github/workflows/ci.yaml) file).

In order to contribute you should make a pull request and ask a person familiar with the codebase for a review.

## Building

The project is a pure Rust project, and can be build by running

```shell
cargo build --release
```

This produces a single binary `target/release/id-verifier`.

## Sample deployment

A full deployment of the verifier with the frontend can be done as follows.

1. Build the frontend as explained in [its README](./frontend/README.md).
2. Build the verifier using `cargo build --release`
3. Run the verifier server, also serving the frontend

```shell
./target/release/id-verifier --node http://node-url:20000 --log-level=debug --dir ./frontend/dist --port 8100
```

This will start the server listening on `0.0.0.0:8100`. The frontend can be accessed by going to `http://localhost:8100/` or `http://localhost:8100/index.html`.
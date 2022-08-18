# Liveness checker.

Check whether the node is sufficiently alive and up to date. This tool is
designed to be used in scripts to automatically check, e.g., that a node has
started up correctly.

# Supported configuration options

The following environment variables (command line options) are supported
- `LIVENESS_CHECKER_NODE` (`--node`) the URL of the node's GRPC interface, e.g., http://localhost:10000
- `LIVENESS_CHECKER_TOKEN` (`--rpc-token`) the token to access the GRPC interface
- `LIVENESS_CHECKER_MAX_FINALIZED_BEHIND` (`--max-behind`) amount of seconds
  that finalization (slot time of the latest finalized block) may be behind present.
- `LIVENESS_CHECKER_MIN_PEERS` (`--min-peers`) Minimum number of peers the node is required to have.
- `LIVENESS_CHECKER_REQUIRE_BAKER` (`--require-baker`) Require that the node is an active baker.

All of the above is available by using `--help` to get usage information.

# Failure handling

The program will start and perform the checks. If all checks pass then nothing
is printed and the program exits with status code 0. If one of the checks fails
the program will print the relevant error to `stderr` and exit with a positive
status code. The meaning of status codes is

- `1` ... could not connect to the node within the alloted time
- `2` ... a query has failed due to an RPC error
- `3` ... a query failed for another reason (this should generally not happen unless the node is completely broken)
- `4` ... the node has not witness finalization since startup
- `5` ... finalization is too far behind
- `6` ... the node has too few peers
- `7` ... the node is expected to be a baker, but it is not

Connection timeout is set to 2 seconds, and request timeout is set to 5 seconds
so the tool should always exit in finite amount of time.

# Contributing

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.0-4baaaa.svg)](https://github.com/Concordium/.github/blob/main/.github/CODE_OF_CONDUCT.md)

This repository's CI automatically checks formatting and common problems in rust.
Changes to any of the packages must be such that
- ```cargo clippy --all``` produces no warnings
- ```rust fmt``` makes no changes.

Everything in this repository should build with stable rust at the moment (at least version 1.56 and up), however the fmt tool must be from a nightly release since some of the configuration options are not stable. One way to run the `fmt` tool is

```shell
 cargo +nightly-2021-06-09 fmt
```
(the exact version used by the CI can be found in [.github/workflows/ci.yaml](https://github.com/Concordium/concordium-misc-tools/blob/main/.github/workflows/ci.yaml) file).
You will need to have a recent enough nightly version installed, which can be done via

```shell
rustup toolchain install nightly-2021-06-09
```
or similar, using the [rustup](https://rustup.rs/) tool. See the documentation of the tool for more details.

In order to contribute you should make a pull request and ask a person familiar with the codebase for a review.

## Building

The project is a pure Rust project, and can be build by running

```shell
cargo build --release
```

This produces a single binary `target/release/concordium-node-liveness-checker`.

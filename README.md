## Collection of miscellaneous tools

This repository contains a collection of small tools with a well-defined
purpose. If a tool is larger in scope, such as the rosetta API, then it should
have its own repository. This is the place for short scripts.

The tools are

- [wallet-proxy-load-simulator](./wallet-proxy-load-simulator)
  A tool to simulate the load on the wallet-proxy similar to what mobile wallets
  are expected to do.
- [state-compare](./state-compare) A tool to compare the state (via the node's
  API) of two blocks, either by the same node, or different nodes.
- [genesis-creator](./genesis-creator) A tool to create genesis files to start
  custom chains.
- [kpi-tracker](./kpi-tracker) A service that collects metrics from a Concordium blockchain and stores them in a database. 
  The data collected is intended for visualization in Grafana.

# Contributing

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.0-4baaaa.svg)](https://github.com/Concordium/.github/blob/main/.github/CODE_OF_CONDUCT.md)

In order to build and check the repository code locally, the following software is required:

- [rustup](https://www.rust-lang.org/tools/install)

To perform the build and checks equivalent to the CI, run these commands locally:

- Check: `cargo check --all-targets --all-features`
- Clippy (includes check): `cargo clippy --all-targets --all-features --no-deps`
- Test: `cargo test --all-features --release`
- Format code: `cargo fmt`

Notice that the nightly version is only used for formatting. Running tests for
`notification-server` requires additional setup, see the [`README.md`](notification-server/README.md)

If a new tool is added to the repository it should be accompanied by
documentation, and put on the list of tools in the README above.

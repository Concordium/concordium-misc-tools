## Collection of miscellaneous tools

This repository contains a collection of small tools with a well-defined
purpose. If a tool is larger in scope, such as the rosetta API, then it should
have its own repository. This is the place for short scripts.

The tools are

- [node-liveness-checker](./node-liveness-checker)
  A tool that determines whether a node is up and running, and whether it is
  sufficiently up to date. The purpose of this tool is to help automate node
  management.
- [wallet-proxy-load-simulator](./wallet-proxy-load-simulator)
  A tool to simulate the load on the wallet-proxy similar to what mobile wallets
  are expected to do.
- [state-compare](./state-compare) A tool to compare the state (via the node's
  API) of two blocks, either by the same node, or different nodes.

# Contributing

[![Contributor Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.0-4baaaa.svg)](https://github.com/Concordium/.github/blob/main/.github/CODE_OF_CONDUCT.md)

This repository's CI automatically checks formatting and common problems in rust.
Changes to any of the packages must be such that
- ```cargo clippy --all``` produces no warnings
- ```rust fmt``` makes no changes.

Everything in this repository should build with stable rust at the moment (at least version 1.53 and up), however the fmt tool must be from a nightly release since some of the configuration options are not stable. One way to run the `fmt` tool is

```shell
 cargo +nightly-2022-06-09 fmt
```
(the exact version used by the CI can be found in [.github/workflows/ci.yaml](.github/workflows/ci.yaml) file).
You will need to have a recent enough nightly version installed, which can be done via

```shell
rustup toolchain install nightly-2022-06-09
```
or similar, using the [rustup](https://rustup.rs/) tool. See the documentation of the tool for more details.

In order to contribute you should make a pull request and ask a person familiar
with the codebase for a review.

If a new tool is added to the repository it should be accompanied with
documentation, and put on the list of tools in the README above.

## Wallet-proxy load simulator

This utility simulates the main query the mobile wallet makes when displaying
accounts. It is intended for load testing the wallet backend.

Currently the utility issues `accBalance` queries. This is the main query that
the mobile wallet uses to ascertain the current balance of the account, as well
as other account information, such as locked balance or pending releases.

The tool supports the following configuration options
- `WP_LOAD_SIMULATOR_URL` (option `--wp-url`) the base URL of the wallet proxy
- `WP_LOAD_SIMULATOR_ACCOUNTS` (option `--accounts`) path to the file with a list of accounts to query. The list is
  expected to be a valid JSON list of account addresses, such as the one
  returned by `concordium-client raw GetAccountList`.
- `WP_LOAD_SIMULATOR_MAX_PARALLEL` (option `--max-parallel`) Maximum number of queries that will be made
  in parallel. This is in general limited by the amount of open connections the
  operating system allows.
- `WP_LOAD_SIMULATOR_DELAY` (option `--delay`) The delay in milliseconds between issuing requests per worker.
- `WP_LOAD_SIMULATOR_TIMEOUT` (option `--timeout`) The timeout per request in
  milliseconds. Requests are cancelled after timeout and deemed to be failed.

All of the above is available by using `--help` to get usage information. An
example invocation will thus look like
```console
wallet-proxy-load-simulator --wp-url http://wallet-proxy.stagenet.concordium.com --accounts accounts.json --delay 100 --max-parallel 100
```

The tool outputs to `stdout`. Each line contains the 
- `i`, the identifier of the worker that sent the request
- `url` the path of the URL that was queried
- `diff` the time in milliseconds until the server responded
- `code`, the HTTP status code of the response, or if there is a network error,
  `0`.

# Contributing

[![Contributor
Covenant](https://img.shields.io/badge/Contributor%20Covenant-2.0-4baaaa.svg)](https://github.com/Concordium/.github/blob/main/.github/CODE_OF_CONDUCT.md)

This repository's CI automatically checks formatting and common problems in rust.
Changes to any of the packages must be such that
- ```cargo clippy --all``` produces no warnings
- ```cargo fmt``` makes no changes.

Everything in this repository should build with stable rust at the moment (at least version 1.56 and up).

In order to contribute you should make a pull request and ask a person familiar with the codebase for a review.

## Building

The project is a pure Rust project, and can be build by running

```shell
cargo build --release
```

This produces a single binary `target/release/wallet-proxy-load-simulator`.


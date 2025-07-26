# Test Bench

A test bench smart contract for testing mobile wallets (via walletConnect) or the browser wallet via a front end.

## Prerequisites

-   `Rust` and `cargo-concordium` need to be installed (developer documentation)[https://developer.concordium.software/en/mainnet/smart-contracts/guides/quick-start.html].


## Setup

Clone the repo:

```shell
git clone --recursive-submodules git@github.com:Concordium/concordium-misc-tools
```

Navigate into this folder:
```shell
cd ../wallet-connect-test-bench/smart-contract
```
todo ar test this, profile must be smart-contract, cargo concordium uses --release

Building the smart contract (with embedded schema) or running the tests, use the following commands:

```shell
cargo concordium build -e
```

```shell
cargo concordium test
```


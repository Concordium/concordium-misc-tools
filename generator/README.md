# Transaction generator.

This is a testing tool used to generate transactions for testing the chain in
different configurations.

## Build

To build it

- make sure to check out git submodules
  ```console
  git submodule update --init --recursive
  ```
- Run the build
  ```console
  cargo build --release
  ```

This will produce a single binary `./target/release/generator`.

## Using the generator

The generator connects to the node and sends transactions. The number of
transactions sent is configured using the `--tps` flag. The `--sender` flag specifies the path to the sender account credentials.

Both TLS and unencrypted connection to the node are supported. Use `https` as
the node address scheme to connect via TLS.

The tool supports multiple kinds of transactions that are given with subcommands. E.g. to mint 5 NFTs a second you would use the command

```console
./generator --tps 5 --sender path/to/keys.json mint-nfts
```

The supported transactions are listed below.

### `ccd`

The transactions are regular CCD transfers sent to

- either accounts specified using the `--receivers` argument which should point
  to a file with a list of account addresses.
- or if that is not supplied to all accounts on the chain (obtained from the node)

In addition to the account list the `--mode` flag determines which accounts and
in which order transactions are being sent to.

If `mode` is `random` then transactions are sent to accounts from the list in
random order.

If `mode` is an integer then the list of receivers is partitioned into the given
amount of accounts. The generator must be connected to a baker, and it is only
sending to the accounts from `i`'th element of the partition, where `i` is the
baker id.

### `mint-nfts`

The tool first deploys and initializes the [`cis2-nft`](https://github.com/Concordium/concordium-rust-smart-contracts/tree/fcc668d87207aaf07b43f5a3b02b6d0a634368d0/examples/cis2-nft) example contract. The transactions are then simply `mint` updates on the contract, where NFTs are minted for the sender.

### `transfer-cis2`

The tool first deploys and initializes the [`cis2-multi`](https://github.com/Concordium/concordium-rust-smart-contracts/tree/fcc668d87207aaf07b43f5a3b02b6d0a634368d0/examples/cis2-nft) example contract. It then mints `u64::MAX` CIS2 tokens for the sender. The transactions are then transfers of these tokens to a list of receivers that is either

- accounts specified using the `--receivers` argument which should point
  to a file with a list of account addresses,
- or every account on the chain if `--receivers` is not specified.

The transactions are sent in a round robin fashion.

### `wccd`

The tool first deploys and initializes the [`cis2-wccd`](https://github.com/Concordium/concordium-rust-smart-contracts/tree/fcc668d87207aaf07b43f5a3b02b6d0a634368d0/examples/cis2-wccd) example contract. It then starts minting 1 (micro) wCCD for each account on the chain in order to increase the size of the state of the contract. After that, the transactions alternate between wrapping, transferring, and unwrapping wCCD. In each case the receiver is the sender, since it is simple and there is no special handling of this in the contract.

### `register-credentials`

The tool first deploys and initializes the [`credential-registry`](https://github.com/Concordium/concordium-rust-smart-contracts/tree/fcc668d87207aaf07b43f5a3b02b6d0a634368d0/examples/credential-registry) example contract. Each transaction is simply an issuance of a credential with dummy values.

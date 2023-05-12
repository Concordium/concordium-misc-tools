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
transactions sent is configured using the `--tps` flag.

The transactions are sent to

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

Both TLS and unencrypted connection to the node are supported. Use `https` as
the node address scheme to connect via TLS.

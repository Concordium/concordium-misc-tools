# State compare

Compares the state of two blocks. The main purpose of this tool is to inspect the
state just before and after the protocol update, but in principle any two blocks
can be supplied to print differences.

# Supported configuration options

The following environment variables (command line options) are supported
- `STATE_COMPARE_NODE1` (`--node1`) (required) the URL of the node's GRPC V2 interface, e.g., http://localhost:20000
- `STATE_COMPARE_NODE2` (`--node2`) (optional, defaults to the `node1` value if
  not given) the URL of the node's GRPC V2 interface, e.g., http://localhost:20000
- `STATE_COMPARE_NODE1` (`--block1`) (optional) the first block to compare the
  state in
- `STATE_COMPARE_NODE2` (`--block2`) (optional) the first block to compare the
  state in

If `block1` is not provided it defaults to the last block before the last
protocol update. If no protocol update has taken effect this is the genesis block.

If `block2` is not provided it defaults to the current era genesis block.

All of the above information is available by using `--help` to get usage
information.

# State checks

The following checks are performed and diffs are printed for any difference found:

- The list of accounts in the two blocks are the same.
- For each of the accounts in the list of accounts, the accounts are the same.
- The list of smart contract modules is the same in both blocks, and the modules
  can be retrieved from both blocks, and their source code matches.
- The list of smart contract instances is the same in both blocks, and for each
  instance both the instance metadata is the same, and the state of each
  instance is the same.
- Passive delegators agree in both blocks. This means the list of them is the
  same, and the amounts are the same.
- Active bakers agree. In particular the election difficulty is the same, and
  the bakers are the same, and have the same lottery power.
- Baker pools are checked. The tool checks that the pools are the same, and have
  the same capital. It also checks that the delegators for each pool are the same,
  which includes checking that the staked amounts are the same, as well as any
  pending change.
- Update sequence numbers are migrated correctly.

The tool will exit with a non-zero status code if it fails to query some data.

## Caveats

The state is checked using the node's API, so this is not a completely
comprehensive diff. The output is meant to be inspected manully to ensure
that the changes make sense with regards to any protocol update.

The tool at present requires a decent amount of memory since it loads the list
of all accounts in memory, and also queries accounts and contract state in
parallel. When this becomes an issue we can limit the amount of concurrency and
do things in a more streaming fashion to reduce resource usage.

## Building

The project is a pure Rust project, and can be built by running

```shell
cargo build --release
```

The tool should build with at least rust 1.65 or later.

This produces a single binary `target/release/concordium-state-compare`.

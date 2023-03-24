# State compare

Check the state of two blocks. The main purpose of this tool is to check the
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

The following checks are performed.

- The list of accounts in the two blocks are the same.
- For each of the accounts in the list of accounts, the accounts are the same
  modulo changes from P3 to P4. If `block1` is in P3 and `block2` is in P4 then
  the equality check for accounts is relaxed and we allow that in `block2` an
  account has baker pool information that it does not have in `block1`.
- The list of smart contract modules is the same in both blocks, and the modules
  can be retrieved from both blocks, and their source code matches.
- The list of smart contract instances is the same in both blocks, and for each
  instance both the instance metadata is the same, and the state of each
  instance is the same.
- Passive delegators agree in both blocks. This means the list of them is the
  same, and the amounts are the same.
- Active bakers agree. In particular the election difficulty is the same, and
  the bakers are the same, and have the same lottery power.
- If both blocks are in protocols P4 and later then baker pools are checked. The
  tool checks that the pools are the same, and have the same capital. It also
  checks that the delegators for each pool are the same, which includes checking
  that the staked amounts are the same, as well as any pending change.

## Example

On mainnet running the tool when protocol version 4 is in effect leads to

```
$ concordium-state-compare --node1 http://localhost:20000
Comparings state in blocks 5af81a1cc51141617f13c37e1cdea7ebdd76fce7e6377c0e7576b7450724472a (protocol version P3) and ea4a52a04ba905c2692b4598aa344707327781d588573d374125b449a1ce0bcc (protocol version P4).
Comparing account lists.
Querying all accounts.
Comparing all modules.
Querying all contracts.
Checking passive delegators.
Checking active bakers.
Checking baker pools.
Not comparing baker pools since one of the protocol versions is before P4.
No changes in the state detected.
```

If two other blocks are chosen an example output shows what differs. For example
on mainnet

```
$ concordium-state-compare --node1 http://localhost:20000 --block1 c58f6582361589dec16d07631081be5446e58b8e3c4f13b82b86d30f2f523706 --block2 abdebbfe582744e6e55ca43dfd4aa81f74d00e3e9010af5d9717203b26fddf39
Comparings state in blocks c58f6582361589dec16d07631081be5446e58b8e3c4f13b82b86d30f2f523706 (protocol version P4) and abdebbfe582744e6e55ca43dfd4aa81f74d00e3e9010af5d9717203b26fddf39 (protocol version P4).
Comparing account lists.
Querying all accounts.
Account 35CJPZohio6Ztii2zy1AYzJKvuxbGG44wrBn7hLHiYLoF2nxnh differs. It does not have stake either in c58f6582361589dec16d07631081be5446e58b8e3c4f13b82b86d30f2f523706 or abdebbfe582744e6e55ca43dfd4aa81f74d00e3e9010af5d9717203b26fddf39.
Comparing all modules.
Querying all contracts.
Checking passive delegators.
Checking active bakers.
Checking baker pools.
Pool 3 differs.
Error: States in the two blocks c58f6582361589dec16d07631081be5446e58b8e3c4f13b82b86d30f2f523706 and abdebbfe582744e6e55ca43dfd4aa81f74d00e3e9010af5d9717203b26fddf39 differ.
```

The tool will exit with a non-zero status code if it fails to query some data,
or there is a difference in state.

## Caveats

The state is checked using the node's API, so this is not a completely
comprehensive check. However it provides a basic check that the state has been
migrated correctly. The output is meant to indicate if there are issues, the
tool does not print the exact difference between the states.

The tool at present requires a decent amount of memory since it loads the list
of all accounts in memory, and also queries accounts and contract state in
parallel. When this becomes an issue we can limit the amount of concurrency and
do things in a more streaming fashion to reduce resource usage.

## Building

The project is a pure Rust project, and can be built by running

```shell
cargo build --release
```

The tool should build with at least rust 1.64 or later.

This produces a single binary `target/release/concordium-state-compare`.

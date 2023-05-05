## recover id object

A command-line tool that implements the identity recovery. It generates recovery
requests, contacts the identity provider, and recovers the identity objects.

## Build

To build run `cargo build --release`. This produces the binary `target/release/recover-id-object`.

## Run

See `--help` for the list of all the options. The tool requires access to the
seed phrase to be used for recovery.

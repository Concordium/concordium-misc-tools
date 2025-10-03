## Unreleased changes

- Update `concordium-rust-sdk` dependency and adjust project to be forward-compatible. Unkown transaction summaries, node details, and unkown node consensus status will produce an error.
- Updated generator with `plt` and `create-plt` arguments

## 1.2.0

- Updated the Concordium Rust SDK to support the changes introduced in protocol 7.
- Add new `register-data` mode for sending Register Data transactions.

## 1.1.1

Stop `wccd` mode from minting to everyone faster than the specified TPS.

## 1.1.0

Support protocol 6 and node version 6.

## 1.0.0

Initial version ported from the rust SDK.

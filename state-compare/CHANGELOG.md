# Changelog for the the state compare tool

## Unreleased changes

- Update `rust-sdk` dependency and adjust project to be forward-compatible.

## 3.0.0

- Updated the Concordium Rust SDK to support the changes introduced in protocol 9. Changes in Protocol 9 include the ability to transfer, mint/burn, add/remove from allow and deny lists and pause and unpause a PLT. This change compares PLT state which is not compatible with a node version <= Protocol 8.

## 2.0.0

- Updated the Concordium Rust SDK to support the changes introduced in protocol 7.
- Reworked the tool so that it merely prints a diff without trying to determine if the differences are expected or not.

## 1.1.1

- Reduce the amount of concurrency.
- Ignore order of transaction hashes when releases are at the same time in
  account comparison.

## 1.1.0

- Add support for protocol version 6.

## 1.0.0

- Initial version.

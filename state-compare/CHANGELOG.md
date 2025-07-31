# Changelog for the the state compare tool

## 3.0.0
- Updated the Concordium Rust SDK to support the changes introduced in protocol 9.

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

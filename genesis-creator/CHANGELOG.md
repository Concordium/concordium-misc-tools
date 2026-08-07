## Unreleased changes

- Add P11 genesis generation and assembly support for `tokenParameters` governance authorization and the `maxLockDuration` chain parameter.
- Reject missing P11 fields and P11-only fields supplied for P8–P10 through the SDK's protocol validation.
- Add a complete P11 example and generation/assembly smoke coverage.

## 0.8.2

- Fix a bug in the computation of the genesis hash for protocol version 11.

## 0.8.0 and 0.8.1

- Support genesis data format of protocol version 11.

## 0.7.0

- Support genesis data format of protocol version 10.

## 0.6.0

- Update `concordium-rust-sdk` dependency so that project is forward compatible.

## 0.5.0

- Support genesis data format of protocol version 9.

## 0.4.0

- Support genesis data format of protocol version 8.

## 0.3.0

- Support genesis data format of protocol version 7.
- Updated the Concordium Rust SDK to support the changes introduced in protocol 7.

## 0.2.0

- Support genesis data format of protocol version 6.
- Change config field `protocolVersion` to be a string representation of the number, due to a limitation in the tooling.

# 0.1.0

Initial release

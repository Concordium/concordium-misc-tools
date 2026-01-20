# Changelog for the kpi-tracker service

## Unreleased changes

## 2.1.6

- Update `concordium-rust-sdk` dependency and adjust project to be forward-compatible. Unknown transaction types, transaction effects, transaction outcomes, block item details, unknown open status values of validator pools or smart contract versions will produce an error as well as when the functions `affected_contracts`/`affected_accounts` return unknown element.

## 2.1.5

- Updated the Concordium Rust SDK to support the changes introduced in protocol 9.

## 2.1.4

- Release environment variable names update

## 2.1.3

- Updated the Concordium Rust SDK to support the changes introduced in protocol 8.

## 2.1.2

- Updated the Concordium Rust SDK to support the changes introduced in protocol 7.

## 2.0.0

- Split transaction graphs into separate types of transactions.
- Add graph for accounts with CCD transfers.
- Add graph for number of active finalizers.
- Add graph showing active staking accounts, divided into bakers and delegators.
- Add graph showing status of active bakers.
- Add graph of delegation recipients.
- Add graph for minted CCDs.
- Add graphs for baker, finalizer, and foundation rewards.
- Add `KPI_TRACKER_BULK_INSERT_MAX` option to insert multiple blocks at the same
  time during catchup. The default is 20.

## 1.1.0

- Support protocol version 6.

## 1.0.0

- Initial version of the service.

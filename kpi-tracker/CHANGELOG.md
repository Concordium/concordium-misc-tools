# Changelog for the kpi-tracker service

## 2.1.1
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

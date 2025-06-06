# Notification api

API to obtain information about particular account to device mappings and store them in a persistence layer.

## Component Interaction Diagram

![Component Interaction Diagram](docs/diagrams/notification-server.drawio.png)

# Notification service

Service indexing the CCD chain and given incoming transactions, emit a notification to the device
associated with the account that received the transaction.

## Setting up local dev

```shell
make setup && make
```

where `make setup` will be a onetime setup and `make` will be continuously used to ensure containers are valid.

and run the application with 

```shell
cargo run --bin <BINARY_NAME>
```

where `<BINARY_NAME>` is the name of the binary you want to run.

## API subscribe documentation

The subscription endpoint is idempotent.

All account-to-device mappings being registered under a single endpoint call will have the same preferences set.
Accounts is a list of base58 encoded account addresses.

At most 1000 accounts can be registered in one call and only 1000 accounts be queried at once.

Should conflicts occur upon subscription updates, then only the preferences are updated.

### Example subscribe request

```shell
curl -X PUT "http://localhost:3030/api/v1/subscription" \
    -H "Content-Type: application/json" \
    -d '{
        "preferences": ["cis2-tx", "ccd-tx"],
        "accounts": ["4FmiTW2L2AccyR9VjzsnpWFSAcohXWf7Vf797i36y526mqiEcp"],
        "device_token": "<device_token>"
    }'
```

### Example unsubscribe request
```shell
curl -X POST "http://localhost:3030/api/v1/unsubscribe" \
    -H "Content-Type: application/json" \
    -d '{
        "device_token": "<device_token>"
    }'
```

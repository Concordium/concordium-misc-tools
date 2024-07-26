# Notification server

Server to obtain information about particular account to device mappings and store them in a persistence layer.

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

All account to device mapping being registered under a single endpoint call will have the same preferences

At most 1000 preferences and accounts can be registered in one call and only a 1000 accounts be queried at once.

Should conflicts occur upon subscription updates, then only the preferences becomes updates.

### Example:

```shell
❯ curl -X PUT "http://localhost:3030/api/v1/device/example-device/subscription" \
    -H "Content-Type: application/json" \
    -d '{
        "preferences": ["CIS2", "CCDTransaction"],
        "accounts": ["6zLVntGxRRgFnwQf4HBZTwK2qWrg3"]
    }'
```
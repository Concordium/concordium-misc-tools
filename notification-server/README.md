# Notification server

Server to obtain information about particular account to device mappings and store them in a persistence layer.

# Notification service

Service running to browse the CCD chain and given incoming transactions, emit a notification to the device
associated with the account that received the transaction.

## Setting up local dev

```shell
make
```

and run the application with 

```shell
cargo run --bin <BINARY_NAME>
```

where `<BINARY_NAME>` is the name of the binary you want to run.
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
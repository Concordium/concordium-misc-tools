# KPI-tracker

This folder contains the source code for the KPI-tracker service used to collect relevant metrics from the chain.

## Dependencies

To run the service successfully, the following dependencies are required to be available:

- Rust v1.65
- PostgreSQL with database to be used in configuration.
- Grafana (not required to run the service, but needed to view the data as intended).

## Initial setup

Do the following to install the project

To run the project, it's required to have access to a postgres database to be used by the service. Default configuration tries to access a database with name `kpi-tracker` on `localhost:5432` with user `postgres` and password `password`. The necessary tables for storing the data will be created by the service.

For development purposes a transient database can be run using docker as

`docker run -e POSTGRES_PASSWORD=password -p 5432:5432 postgres`

## Build and run

- Build the service with `cargo build`.
- Run the service in the terminal with `cargo run --` followed by any command line arguments required.

### Runtime configuration

The service can be configured with a number of runtime arguments:

- `--node` (environment variable `KPI_TRACKER_NODES`), which takes a list of node GRPC2 endpoints consisting of both host and port. Default value is `http://localhost:20001`. If the URL starts with `https` schema then the tool will establish TLS connection to the node, using the system trust roots.
- `--db-connection` (environment variable `KPI_TRACKER_DB_CONNECTION`), which takes a database connection string. Default value is `host=localhost dbname=kpi-tracker user=postgres password=password port=5432`.
- `--log-level` (environment variable `KPI_TRACKER_LOG_LEVEL`), which takes a logging level. Possible values are `off`, `trace`, `debug`, `info`, `warn`, `error`. Default value is `debug`.
- `--num-parallel` (environment variable `KPI_TRACKER_NUM_PARALLEL`), which takes an integer specifying the number of parallel queries to be made to a node. Default value is `1`.
- `--max-behind-seconds` (environment variable `KPI_TRACKER_MAX_BEHIND_SECONDS`), which takes an integer specifying the max amount of time in seconds to wait on a response from a node before trying the next in the list.

The default configuration is meant for use in a development environment.

## Database

The database schema can be seen in [resources/schema.sql](./resources/schema.sql).

### Structure

The database is structured in a way that facilitates time-series output for all data collected, by always linking to blocks. The specific entities stored are:

- Blocks in `blocks`
- Accounts in `accounts`
- Contract modules in `modules`
- Contract instances in `contracts`
- Account transactions in `transactions`

Indices are created where needed to improve performance on certain queries. As everything is linked to blocks mainly to join rows with a timestamp, the most critical of these is the index on `blocks.timestamp`.

Furthermore, to store relations between account/contracts and transactions there are two relations tables

- Relations between accounts and transactions in `accounts_transactions`
- Relations between contract instances and transactions in `contracts_transactions`

#### Entity activeness

To support querying accounts and contract instances which have been active, two additional tables have been created. These store derived data (like a materialized view), and only exist to support querying this with reasonable performance.

- Account activeness in `account_activeness`
- Contract instance activeness in `contract_activeness`

These record dates (in timestamps) accounts/contracts have been part of a transaction. The relation between the tables recording the basic entities (accounts, contract instances, transactions, and blocks) and the tables storing the derived data is described in [resources/populate-activeness.sql](./resources/populate-activeness.sql).

## Grafana

We use Grafana to visualize the data collected by the service. The configuration of the dashboard can be found under [grafana/dashboard.json](./grafana/dashboard.json).

### Installation and running

Follow the steps to [install Grafana](https://grafana.com/docs/grafana/latest/setup-grafana/installation/) for your OS, which should also go through the steps required to run the installed service. 

Can also be run using docker with docker using

`docker run -d -p 3000:3000 grafana/grafana-oss`

which runs a grafana server on port `3000` in the background.

### Configuration

- Set up a PostgreSQL data source pointing to your local database, by following [the instructions](https://grafana.com/docs/grafana/latest/datasources/postgres/).
  - It is recommended to set up a user with read-only access to the tables in the database, however for running locally it will be easier just to use the same user/password combination used for the service database connection
- In the Grafana interface under "Dashboards", click "+ Import", and import `grafana/dashboard.json`.

## Docker build

A [Dockerfile](./Dockerfile) is available that produces a self-contained image with the service installed and set as the entrypoint.

This docker image can be built using
```
docker build --build-arg build_image=rust:1.70-buster --build-arg base_image=debian:buster -f ./scripts/Dockerfile ../
```
which produces a debian-buster based image.

## Compose configuration for local development

There is a [compose.yml](./scripts/compose.yml) file that can be used to start
up both a postgres database and grafana loaded with the dashboard to present
KPIs.

To start it use

```console
docker-compose -f ./scripts/compose.yml up
```
from the directory of the README file.

This will start a grafana service listening on port 3000 on the host, and a
postgres database listening on port 5432. The `kpi-tracker` database is created
on startup.

Note that no state is persisted, so when the process terminates all data is
forfeit.

This compose configuration uses the configurations in
[scripts/provisioning](./scripts/provisioning/) to set up the default grafana
dashboard and data source.

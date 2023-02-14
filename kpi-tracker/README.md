# KPI-tracker

This folder contains the source code for the KPI-tracker service used to collect relevant metrics from the chain.

## Dependencies

To run the service successfully, the following dependencies are required to be available:

- Rust v1.62
- PostgreSQL with database to be used in configuration.
- Grafana (not required to run the service, but needed to view the data as intended).

## Initial setup

Do the following to install the project

- Create PostgreSQL database to be used by service. Default configuration tries to access a database with name `kpi-tracker` on `localhost:5432` with user `postgres` and password `password`. The necessary tables for storing the data will be created by the service.

## Build and run

- Build the service with `cargo build`.
- Run the service in the terminal with `cargo run --` followed by any command line arguments required.

### Runtime configuration

The service can be configured with a number of runtime arguments:

- `--node` (environment variable `KPI_TRACKER_NODES`), which takes a list of node GRPC2 endpoints consisting of both host and port. Default value is `http://localhost:20001`.
- `--db-connection` (environment variable `KPI_TRACKER_DB_CONNECTION`), which takes a database connection string. Default value is `host=localhost dbname=kpi-tracker user=postgres password=password port=5432`.
- `--log-level` (environment variable `KPI_TRACKER_LOG_LEVEL`), which takes a logging level. Possible values are `off`, `trace`, `debug`, `info`, `warn`, `error`. Default value is `debug`.
- `--num-parallel` (environment variable `KPI_TRACKER_NUM_PARALLEL`), which takes an integer specifying the number of parallel queries to be made to a node. Default value is `1`.
- `--max-behind-seconds` (environment variable `KPI_TRACKER_MAX_BEHIND_SECONDS`), which takes an integer specifying the max amount of time in seconds to wait on a response from a node before trying the next in the list.

The default configuration is meant for use in a development environment.

## Grafana

We use Grafana to visualize the data collected by the service. The configuration of the dashboard can be found under [grafana/dashboard.json](./grafana/dashboard.json).

### Installation and running

Follow the steps to [install Grafana](https://grafana.com/docs/grafana/latest/setup-grafana/installation/) for your OS, which should also go through the steps required to run the installed service.

### Configuration

- Set up a PostgreSQL data source pointing to your local database, by following [the instructions](https://grafana.com/docs/grafana/latest/datasources/postgres/).
  - It is recommended to set up a user with read-only access to the tables in the database, however for running locally it will be easier just to use the same user/password combination used for the service database connection
- In the Grafana interface under "Dashboards", click "+ Import", and import `grafana/dashboard.json`.

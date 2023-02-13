# KPI-tracker

This folder contains the source code for the KPI-tracker service used to collect relevant metrics from the chain.

## Grafana

We use Grafana to visualize the data collected by the service. The configuration of the dashboard can be found under [grafana/dashboard.json](./grafana/dashboard.json).

### Installation and running

Follow the steps to [install grafana](https://grafana.com/docs/grafana/latest/setup-grafana/installation/) for your OS, which should also go through the steps required to run the installed service.

### Configuration

- Set up a PostgreSQL data source pointing to your local database, by following [the instructions](https://grafana.com/docs/grafana/latest/datasources/postgres/).
  - It is recommended to set up a user with read-only access to the tables in the database, however for running locally it will be easier just to use the same user/password combination used for the service DB connection
- In the Grafana interface under "Dashboards", click "+ Import", and import `grafana/dashboard.json`.

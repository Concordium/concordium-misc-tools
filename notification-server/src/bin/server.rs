use clap::Parser;
use concordium_rust_sdk::v2::{Client, Endpoint};
use futures::StreamExt;
use tonic::codegen::http;
use tonic::transport::ClientTlsConfig;
use warp::{Filter, Reply};
use serde::Deserialize;
use warp::http::StatusCode;

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        default_value = "host=localhost dbname=kpi-tracker user=postgres password=password \
                         port=5432",
        help = "A connection string detailing the connection to the database used by the \
                application.",
    )]
    db_connection:   tokio_postgres::config::Config,
   /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level:       log::LevelFilter,
}


#[derive(Deserialize)]
struct DeviceMapping {
    device_id: String,
}
pub async fn upsert_account_device(
    account: String,
    device_mapping: DeviceMapping,
) -> Result<impl Reply, warp::Rejection> {
    println!("Upserting account {} with device id {}", account, device_mapping.device_id);
    Ok(StatusCode::OK)
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    env_logger::Builder::new()
        .filter_module(module_path!(), args.log_level) // Only log the current module (main).
        .init();
    let account_device_route = warp::path!("api" / "v1" / "account" / String / "device_map")
        .and(warp::put())
        .and(warp::body::json())
        .and_then(upsert_account_device);
    let routes = account_device_route;
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

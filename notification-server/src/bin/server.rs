use clap::Parser;
use serde::Deserialize;
use warp::{http::StatusCode, Filter, Reply};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application."
        env = "DB_CONNECTION"
    )]
    db_connection: tokio_postgres::config::Config,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level:     log::LevelFilter,
}

#[derive(Deserialize)]
struct DeviceMapping {
    device_id: String,
}
async fn upsert_account_device(
    account: String,
    device_mapping: DeviceMapping,
) -> Result<impl Reply, warp::Rejection> {
    println!(
        "Upserting account {} with device id {}",
        account, device_mapping.device_id
    );
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

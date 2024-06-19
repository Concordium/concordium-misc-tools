use clap::Parser;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use dotenv::dotenv;
use log::info;
use serde::Deserialize;
use tokio_postgres::{Config, NoTls};
use warp::{http::StatusCode, Filter, Reply};

#[derive(Debug, Parser)]
struct Args {
    #[arg(
        long = "db-connection",
        help = "A connection string detailing the connection to the database used by the \
                application.",
        env = "DB_CONNECTION"
    )]
    db_connection: tokio_postgres::config::Config,
    /// Logging level of the application
    #[arg(long = "log-level", default_value_t = log::LevelFilter::Info)]
    log_level:     log::LevelFilter,
}

#[derive(Deserialize)]
struct Device {
    id: String,
}
async fn upsert_account_device(
    account: String,
    device: Device,
    pool: Pool
) -> Result<impl Reply, warp::Rejection> {
    info!("Creating device: {} for account {}", account, device.id);
        let query = "
        INSERT INTO account_device_mapping (address, device_id)
        VALUES ($1, $2)
        ON CONFLICT (address) DO UPDATE SET device_id = EXCLUDED.device_id
    ";
    // TODO fix unwrap
    let db_client = pool.get().await.unwrap();
    db_client.execute(query, &[&account, &device.id])
        .await.unwrap();
    Ok(StatusCode::OK)
}


async fn init_db(config: Config) -> Pool {
    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast
    };
    let mgr = Manager::from_config(config, NoTls, mgr_config);
    Pool::builder(mgr).max_size(16).build().expect("Failing on initialising the database pool.")
}

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();
    let args = Args::parse();
    env_logger::Builder::new()
        .filter_module(module_path!(), args.log_level) // Only log the current module (main).
        .init();
    let pool = init_db(args.db_connection).await;
    let account_device_route = warp::path!("api" / "v1" / "account" / String / "device_map")
        .and(warp::put())
        .and(warp::body::json())
        .and(warp::any().map(move || pool.clone()))
        .and_then(upsert_account_device);
    let routes = account_device_route;
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
    Ok(())
}

use tracing_subscriber::filter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

pub fn init_logging(log_level: filter::LevelFilter) -> anyhow::Result<()> {
    let filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::builder().from_env_lossy()
    } else {
        let pkg_name = env!("CARGO_PKG_NAME").replace('-', "_");
        let crate_name = env!("CARGO_CRATE_NAME");
        format!("info,{pkg_name}={0},{crate_name}={0}", log_level).parse()?
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(filter)
        .init();

    Ok(())
}
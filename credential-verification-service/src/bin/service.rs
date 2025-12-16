use clap::Parser;

use credential_verification_service::{configs::ServiceConfigs, logging, service};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let configs = ServiceConfigs::parse();

    logging::init_logging(configs.log_level)?;

    service::run(configs).await?;
    Ok(())
}

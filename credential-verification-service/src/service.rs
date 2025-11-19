use concordium_rust_sdk::{
    types::WalletAccount,
    v2::{self, Client},
};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::{api, configs::ServiceConfigs};

pub async fn run(configs: ServiceConfigs) -> anyhow::Result<()> {
    let client = Client::new(configs.node).await?;
    let keys = WalletAccount::from_json_file(configs.account)?;
    let service = Service {
        healthy: true,
        client,
        keys,
    };

    let listener = TcpListener::bind(configs.address).await?;
    let cancel_token = CancellationToken::new();
    axum::serve(listener, api::init_routes(service))
        .with_graceful_shutdown(cancel_token.cancelled_owned())
        .await?;
    Ok(())
}

pub struct Service {
    pub healthy: bool,
    pub client: v2::Client,
    pub keys: WalletAccount,
}

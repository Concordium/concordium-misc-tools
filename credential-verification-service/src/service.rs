use crate::node_client::{NodeClient, NodeClientImpl};
use crate::{api, configs::ServiceConfigs, types::Service};
use anyhow::{Context, bail};
use concordium_rust_sdk::{
    constants::{MAINNET_GENESIS_BLOCK_HASH, TESTNET_GENESIS_BLOCK_HASH},
    types::WalletAccount,
    v2::{self},
    web3id::did::Network,
};
use futures_util::TryFutureExt;
use prometheus_client::{metrics, registry::Registry};
use std::sync::Arc;
use tokio::{net::TcpListener, sync::Mutex};
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tonic::transport::ClientTlsConfig;
use tracing::{error, info};

pub async fn run(configs: ServiceConfigs) -> anyhow::Result<()> {
    let endpoint = configs
        .node_endpoint
        .clone()
        .tls_config(ClientTlsConfig::new())
        .context("Unable to construct TLS configuration for Concordium node.")?;

    let node_timeout = std::time::Duration::from_millis(configs.grpc_node_request_timeout);

    let endpoint = endpoint
        .connect_timeout(node_timeout)
        .timeout(node_timeout)
        .keep_alive_while_idle(true);

    let node_client = v2::Client::new(endpoint)
        .await
        .context("Unable to establish connection to the node.")?;
    let node_client = NodeClientImpl::new(node_client);

    run_with_dependencies(configs, node_client.boxed()).await
}

pub async fn run_with_dependencies(
    configs: ServiceConfigs,
    mut node_client: Box<dyn NodeClient>,
) -> anyhow::Result<()> {
    let service_info = metrics::info::Info::new([("version", clap::crate_version!().to_string())]);
    let mut metrics_registry = Registry::default();
    metrics_registry.register("service", "Information about the software", service_info);
    metrics_registry.register(
        "service_startup_timestamp_millis",
        "Timestamp of starting up the API service (Unix time in milliseconds)",
        metrics::gauge::ConstGauge::new(chrono::Utc::now().timestamp_millis()),
    );

    // Load account keys and sender address from a file
    let keys: WalletAccount =
        WalletAccount::from_json_file(configs.account).context("Could not read the keys file.")?;

    let account_keys = Arc::new(keys);

    let nonce = node_client
        .get_next_account_sequence_number(&account_keys.address)
        .await
        .context("get account sequence number")?;

    let genesis_hash = node_client
        .get_genesis_block_hash()
        .await
        .context("get genesis block hash")?;

    let network = match genesis_hash.bytes {
        TESTNET_GENESIS_BLOCK_HASH => Network::Testnet,
        MAINNET_GENESIS_BLOCK_HASH => Network::Mainnet,
        _ => bail!(
            "Only TESTNET/MAINNET supported. Unknown genesis hash: {:?}",
            genesis_hash
        ),
    };

    let service = Arc::new(Service {
        node_client,
        account_keys,
        nonce: Arc::new(Mutex::new(nonce)),
        transaction_expiry_secs: configs.transaction_expiry_secs,
        network,
    });

    let cancel_token = CancellationToken::new();
    let monitoring_task = {
        let listener = TcpListener::bind(configs.monitoring_address)
            .await
            .context("Failed to parse monitoring TCP address")?;
        let stop_signal = cancel_token.child_token();
        info!(
            "Monitoring server is running at {:?}",
            configs.monitoring_address
        );
        axum::serve(
            listener,
            api::monitoring_router(metrics_registry, service.clone()),
        )
        .with_graceful_shutdown(stop_signal.cancelled_owned())
        .into_future()
    };

    let api_task = {
        let listener = TcpListener::bind(configs.api_address)
            .await
            .context("Failed to parse API TCP address")?;
        let stop_signal = cancel_token.child_token();
        info!(
            "API server is running at {:?} with account {} and current account nonce: {}.",
            configs.api_address, service.account_keys.address, nonce
        );

        axum::serve(listener, api::router(service, configs.request_timeout))
            .with_graceful_shutdown(stop_signal.cancelled_owned())
            .into_future()
    };

    let cancel_token_clone = cancel_token.clone();
    tokio::spawn({
        async move {
            tokio::signal::ctrl_c().await.ok();
            info!("Received signal to shutdown");
            cancel_token_clone.cancel();
        }
    });

    let task_tracker = TaskTracker::new();
    let cancel_token_clone = cancel_token.clone();
    task_tracker.spawn(api_task.inspect_err(move |err| {
        error!("REST API server error: {}", err);
        cancel_token_clone.cancel();
    }));

    let cancel_token_clone = cancel_token.clone();
    task_tracker.spawn(monitoring_task.inspect_err(move |err| {
        error!("Monitoring server error: {}", err);
        cancel_token_clone.cancel();
    }));

    task_tracker.close();
    task_tracker.wait().await;

    info!("Service is shut down");

    Ok(())
}

use anyhow::Context;
use concordium_rust_sdk::{
    types::WalletAccount,
    v2::{self, Client},
};
use futures_util::TryFutureExt;
use prometheus_client::metrics;
use prometheus_client::registry::Registry;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio_util::{sync::CancellationToken, task::TaskTracker};
use tracing::{error, info};

use crate::{api, configs::ServiceConfigs};

pub struct Service {
    pub client: v2::Client,
    pub keys: WalletAccount,
}

pub async fn run(configs: ServiceConfigs) -> anyhow::Result<()> {
    let service_info = metrics::info::Info::new([("version", clap::crate_version!().to_string())]);
    let mut metrics_registry = Registry::default();
    metrics_registry.register("service", "Information about the software", service_info);
    metrics_registry.register(
        "service_startup_timestamp_millis",
        "Timestamp of starting up the API service (Unix time in milliseconds)",
        metrics::gauge::ConstGauge::new(chrono::Utc::now().timestamp_millis()),
    );

    let client = Client::new(configs.node_address).await?;
    let keys = WalletAccount::from_json_file(configs.account)?;
    let service = Arc::new(Service { client, keys });

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
        info!("Server is running at {:?}", configs.api_address);

        axum::serve(listener, api::router(service))
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

//! Reecho background service.
//!
//! Manages the hotspot lifecycle via NetworkManager and hostapd.
//! Exposes a D-Bus API for the GNOME Shell extension and CLI.

mod activation;
mod ap;
mod blacklist;
mod config;
mod dbus;
mod devices;
mod error;
mod limits;
mod network;
mod scheduler;

use std::sync::Arc;

use tokio::sync::Mutex;
use zbus::connection::Builder;

use reecho_shared::{DBUS_NAME, DBUS_PATH};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().unwrap()),
        )
        .init();

    tracing::info!("Reecho service starting");

    let cfg = config::Config::load();

    let pipeline: Option<Arc<Mutex<activation::ActivationPipeline>>> = match build_pipeline().await
    {
        Ok(p) => {
            tracing::info!("built real ActivationPipeline");
            Some(Arc::new(Mutex::new(p)))
        }
        Err(e) => {
            tracing::warn!("failed to build real pipeline, running in stub mode: {e}");
            None
        }
    };

    let service = if let Some(p) = pipeline {
        dbus::ReechoService::new_with_pipeline(cfg, p)
    } else {
        dbus::ReechoService::new_with_config(cfg)
    };

    let conn = Builder::session()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_PATH, service.clone())?
        .build()
        .await?;

    service.set_connection(conn.clone()).await;

    tracing::info!("Reecho service ready on {DBUS_NAME}");

    tokio::signal::ctrl_c().await?;
    tracing::info!("shutting down");

    Ok(())
}

async fn build_pipeline() -> Result<activation::ActivationPipeline, Box<dyn std::error::Error>> {
    let nm_ops = Arc::new(network::NetworkManagerClient::new().await?);
    let nm_conn = Arc::new(network::RealNetworkManagerConnection::new().await?);
    let cmd_runner = Arc::new(network::RealCommandRunner);
    let spawner = Arc::new(ap::RealHostapdSpawner);
    let querier = Arc::new(ap::RealHostapdStatusQuerier);

    Ok(activation::ActivationPipeline::new(
        nm_ops, nm_conn, cmd_runner, spawner, querier,
    ))
}

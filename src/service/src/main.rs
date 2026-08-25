//! Reecho background service.
//!
//! Manages the hotspot lifecycle via NetworkManager and hostapd.
//! Exposes a D-Bus API for the GNOME Shell extension and CLI.

mod ap;
mod config;
mod dbus;
mod error;
mod network;

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

    let service = dbus::ReechoService::new();

    let _conn = Builder::session()?
        .name(DBUS_NAME)?
        .serve_at(DBUS_PATH, service)?
        .build()
        .await?;

    tracing::info!("Reecho service ready on {DBUS_NAME}");

    std::future::pending::<()>().await;

    Ok(())
}

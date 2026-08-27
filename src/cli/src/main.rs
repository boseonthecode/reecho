//! Reecho command-line interface.
//!
//! Provides scripting access to the Reecho service via D-Bus.

mod dbus_client;

use clap::{Parser, Subcommand};

/// Reecho — mobile hotspot for Linux.
#[derive(Parser)]
#[command(name = "reecho", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Activate the hotspot.
    On {
        /// Wi-Fi network name (SSID).
        #[arg(short, long)]
        ssid: Option<String>,

        /// Hotspot password (min 8, max 63 characters).
        #[arg(short, long)]
        password: Option<String>,

        /// Wi-Fi band: 2.4GHz or 5GHz.
        #[arg(short, long)]
        band: Option<String>,
    },

    /// Deactivate the hotspot.
    Off,

    /// Show current hotspot status.
    Status,

    /// Manage hotspot configuration.
    Config {
        /// Action: get or set.
        action: String,

        /// Configuration key (ssid, password, band).
        key: Option<String>,

        /// Configuration value (for set).
        value: Option<String>,
    },

    /// List connected devices.
    Devices,

    /// Manage device blacklist.
    Blacklist {
        /// Action: add, remove, or list.
        action: String,

        /// MAC address (for add/remove).
        mac: Option<String>,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let client = match dbus_client::ReechoClient::connect().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error: cannot connect to Reecho service: {e}");
            eprintln!("Is the service running? Start it with: systemctl --user start reecho");
            std::process::exit(1);
        }
    };

    let result = match cli.command {
        Commands::On {
            ssid,
            password,
            band,
        } => cmd_on(&client, ssid, password, band).await,
        Commands::Off => cmd_off(&client).await,
        Commands::Status => cmd_status(&client).await,
        Commands::Config { action, key, value } => cmd_config(&client, &action, key, value).await,
        Commands::Devices => cmd_devices(&client).await,
        Commands::Blacklist { action, mac } => cmd_blacklist(&client, &action, mac).await,
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

async fn cmd_on(
    client: &dbus_client::ReechoClient,
    ssid: Option<String>,
    password: Option<String>,
    band: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get current config to fill in defaults.
    let (cfg_ssid, cfg_password, cfg_band) = client.get_config().await?;

    let ssid = ssid.as_deref().unwrap_or(&cfg_ssid);
    let password = password.as_deref().unwrap_or(&cfg_password);
    let band = band.as_deref().unwrap_or(&cfg_band);

    println!("Activating hotspot...");
    println!("  SSID:   {ssid}");
    println!("  Band:   {band}");

    client.activate(ssid, password, band).await?;

    // Wait a moment and check state.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let state = client.get_state().await?;
    if state == "active" {
        println!("Hotspot is now active.");
        let warning = client.get_warning().await?;
        if !warning.is_empty() {
            eprintln!("Warning: {warning}");
        }
    } else {
        println!("Hotspot state: {state}");
    }

    Ok(())
}

async fn cmd_off(client: &dbus_client::ReechoClient) -> Result<(), Box<dyn std::error::Error>> {
    println!("Deactivating hotspot...");
    client.deactivate().await?;
    println!("Hotspot deactivated.");
    Ok(())
}

async fn cmd_status(client: &dbus_client::ReechoClient) -> Result<(), Box<dyn std::error::Error>> {
    let state = client.get_state().await?;
    let warning = client.get_warning().await?;

    println!("Hotspot status: {state}");

    if state == "active" {
        let (cfg_ssid, _, cfg_band) = client.get_config().await?;
        println!("  SSID: {cfg_ssid}");
        println!("  Band: {cfg_band}");

        let devices = client.get_devices().await?;
        println!("  Devices: {}", devices.len());

        let (rx, tx, limit) = client.get_data_usage().await?;
        println!("  Data RX: {}", format_bytes(rx));
        println!("  Data TX: {}", format_bytes(tx));
        if limit > 0 {
            println!("  Data limit: {}", format_bytes(limit));
        }
    }

    if !warning.is_empty() {
        println!("  Warning: {warning}");
    }

    Ok(())
}

async fn cmd_config(
    client: &dbus_client::ReechoClient,
    action: &str,
    key: Option<String>,
    value: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "get" => {
            let (ssid, password, band) = client.get_config().await?;
            match key.as_deref() {
                Some("ssid") => println!("{ssid}"),
                Some("password") => println!("{password}"),
                Some("band") => println!("{band}"),
                Some(k) => {
                    eprintln!("Unknown config key: {k}");
                    eprintln!("Valid keys: ssid, password, band");
                    std::process::exit(1);
                }
                None => {
                    println!("  ssid:     {ssid}");
                    println!("  password: {password}");
                    println!("  band:     {band}");
                }
            }
        }
        "set" => {
            let key = key.ok_or("config set requires a key")?;
            let value = value.ok_or("config set requires a value")?;

            let (mut ssid, mut password, mut band) = client.get_config().await?;
            match key.as_str() {
                "ssid" => ssid = value,
                "password" => password = value,
                "band" => band = value,
                k => {
                    eprintln!("Unknown config key: {k}");
                    eprintln!("Valid keys: ssid, password, band");
                    std::process::exit(1);
                }
            }
            client.set_config(&ssid, &password, &band).await?;
            println!("Config updated.");
        }
        _ => {
            eprintln!("Unknown config action: {action}");
            eprintln!("Valid actions: get, set");
            std::process::exit(1);
        }
    }
    Ok(())
}

async fn cmd_devices(client: &dbus_client::ReechoClient) -> Result<(), Box<dyn std::error::Error>> {
    let devices = client.get_devices().await?;

    if devices.is_empty() {
        println!("No devices connected.");
        return Ok(());
    }

    println!(
        "{:<20} {:<16} {:<20} {:>10} {:>10}",
        "MAC", "IP", "Name", "RX", "TX"
    );
    println!("{}", "-".repeat(80));

    for (mac, ip, name, _connected_at, bytes_rx, bytes_tx, _rate) in &devices {
        println!(
            "{:<20} {:<16} {:<20} {:>10} {:>10}",
            mac,
            ip,
            name,
            format_bytes(*bytes_rx as u64),
            format_bytes(*bytes_tx as u64),
        );
    }

    println!("\n{} device(s) connected.", devices.len());
    Ok(())
}

async fn cmd_blacklist(
    client: &dbus_client::ReechoClient,
    action: &str,
    mac: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "add" => {
            let mac = mac.ok_or("blacklist add requires a MAC address")?;
            client.blacklist_device(&mac).await?;
            println!("Added {mac} to blacklist.");
        }
        "remove" => {
            let mac = mac.ok_or("blacklist remove requires a MAC address")?;
            client.unblacklist_device(&mac).await?;
            println!("Removed {mac} from blacklist.");
        }
        "list" => {
            // The service doesn't have a GetBlacklist method yet,
            // so we'll show a placeholder.
            println!("Blacklist (use 'reecho blacklist add/remove' to manage):");
            println!("(full list not yet available via D-Bus)");
        }
        _ => {
            eprintln!("Unknown blacklist action: {action}");
            eprintln!("Valid actions: add, remove, list");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Format bytes into human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn format_bytes_kb() {
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn format_bytes_mb() {
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0 MB");
    }

    #[test]
    fn format_bytes_gb() {
        assert_eq!(format_bytes(2 * 1024 * 1024 * 1024), "2.0 GB");
    }
}

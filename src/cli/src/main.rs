//! Reecho command-line interface.
//!
//! Provides scripting access to the Reecho service via D-Bus.

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

        /// Configuration key.
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::On {
            ssid,
            password,
            band,
        } => {
            println!("Activating hotspot...");
            if let Some(s) = ssid {
                println!("  SSID: {s}");
            }
            if let Some(p) = password {
                println!("  Password: {p}");
            }
            if let Some(b) = band {
                println!("  Band: {b}");
            }
            println!("(not yet implemented — will connect to service via D-Bus)");
        }
        Commands::Off => {
            println!("Deactivating hotspot...");
            println!("(not yet implemented — will connect to service via D-Bus)");
        }
        Commands::Status => {
            println!("Hotspot status:");
            println!("(not yet implemented — will query service via D-Bus)");
        }
        Commands::Config { action, key, value } => {
            println!("Config {action}...");
            if let Some(k) = key {
                println!("  Key: {k}");
            }
            if let Some(v) = value {
                println!("  Value: {v}");
            }
            println!("(not yet implemented — will connect to service via D-Bus)");
        }
        Commands::Devices => {
            println!("Connected devices:");
            println!("(not yet implemented — will query service via D-Bus)");
        }
        Commands::Blacklist { action, mac } => {
            println!("Blacklist {action}...");
            if let Some(m) = mac {
                println!("  MAC: {m}");
            }
            println!("(not yet implemented — will connect to service via D-Bus)");
        }
    }
}

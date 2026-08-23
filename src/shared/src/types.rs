use serde::{Deserialize, Serialize};

/// Hotspot operational state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HotspotState {
    Inactive,
    Activating,
    Active,
    Failed,
}

impl std::fmt::Display for HotspotState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Inactive => write!(f, "inactive"),
            Self::Activating => write!(f, "activating"),
            Self::Active => write!(f, "active"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Wi-Fi band selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Band {
    Band2_4Ghz,
    Band5Ghz,
}

impl std::fmt::Display for Band {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Band2_4Ghz => write!(f, "2.4GHz"),
            Self::Band5Ghz => write!(f, "5GHz"),
        }
    }
}

impl std::str::FromStr for Band {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "2.4GHz" | "2.4ghz" | "2.4" => Ok(Self::Band2_4Ghz),
            "5GHz" | "5ghz" | "5" => Ok(Self::Band5Ghz),
            _ => Err(format!("invalid band: {s}")),
        }
    }
}

/// Hotspot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotspotConfig {
    pub ssid: String,
    pub password: String,
    pub band: Band,
    pub enabled: bool,
}

/// A device connected to the hotspot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectedDevice {
    pub mac: String,
    pub ip: String,
    pub name: String,
    pub connected_at: String,
    pub bytes_rx: u64,
    pub bytes_tx: u64,
    pub rate_rx: f64,
    pub rate_tx: f64,
}

/// Cumulative data usage tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUsage {
    pub total_rx: u64,
    pub total_tx: u64,
    pub limit: u64,
    pub started_at: String,
}

/// Schedule entry for auto on/off.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    pub on_time: String,
    pub off_time: String,
    pub repeat: String,
    pub enabled: bool,
}

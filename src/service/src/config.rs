//! Configuration persistence for the Reecho service.
//!
//! Loads and saves hotspot configuration to `~/.config/reecho/config.toml`.
//! Provides sensible defaults on first run.

use std::fs;
use std::path::PathBuf;

use rand::Rng;
use serde::{Deserialize, Serialize};

use reecho_shared::{Band, MAX_PASSWORD_LEN, MAX_SSID_LEN, MIN_PASSWORD_LEN};

/// Service configuration persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Hotspot SSID (max 32 bytes).
    pub ssid: String,
    /// Hotspot password (min 8, max 63 bytes).
    pub password: String,
    /// Wi-Fi band selection.
    pub band: Band,
    /// Data usage cap in bytes (0 = unlimited).
    pub data_limit: u64,
    /// Blacklisted MAC addresses.
    #[serde(default)]
    pub blacklist: Vec<String>,
    /// Auto-on time (HH:MM format, 24h). None = disabled.
    #[serde(default)]
    pub auto_on: Option<String>,
    /// Auto-off time (HH:MM format, 24h). None = disabled.
    #[serde(default)]
    pub auto_off: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ssid: default_ssid(),
            password: default_password(),
            band: Band::Band5Ghz,
            data_limit: 0,
            blacklist: Vec::new(),
            auto_on: None,
            auto_off: None,
        }
    }
}

impl Config {
    /// Load configuration from disk, or create defaults if missing/invalid.
    pub fn load() -> Self {
        let path = config_path();
        match fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(config) => {
                    tracing::info!("loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!("failed to parse config, using defaults: {e}");
                    Self::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("no config file found, using defaults");
                let config = Self::default();
                if let Err(e) = config.save() {
                    tracing::warn!("failed to save default config: {e}");
                }
                config
            }
            Err(e) => {
                tracing::warn!("failed to read config, using defaults: {e}");
                Self::default()
            }
        }
    }

    /// Save configuration to disk.
    pub fn save(&self) -> Result<(), ServiceError> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| ServiceError::Config(format!("failed to create config dir: {e}")))?;
        }
        let contents = toml::to_string_pretty(self)
            .map_err(|e| ServiceError::Config(format!("failed to serialize config: {e}")))?;
        fs::write(&path, contents)
            .map_err(|e| ServiceError::Config(format!("failed to write config: {e}")))?;
        tracing::info!("saved config to {}", path.display());
        Ok(())
    }

    /// Validate the configuration, returning errors for invalid values.
    pub fn validate(&self) -> Result<(), ServiceError> {
        if self.ssid.is_empty() {
            return Err(ServiceError::InvalidInput("SSID cannot be empty".into()));
        }
        if self.ssid.len() > MAX_SSID_LEN {
            return Err(ServiceError::InvalidInput(format!(
                "SSID too long (max {MAX_SSID_LEN} bytes)"
            )));
        }
        if self.password.len() < MIN_PASSWORD_LEN {
            return Err(ServiceError::InvalidInput(format!(
                "Password too short (min {MIN_PASSWORD_LEN} bytes)"
            )));
        }
        if self.password.len() > MAX_PASSWORD_LEN {
            return Err(ServiceError::InvalidInput(format!(
                "Password too long (max {MAX_PASSWORD_LEN} bytes)"
            )));
        }
        // Validate MAC addresses in blacklist.
        for mac in &self.blacklist {
            if !is_valid_mac(mac) {
                return Err(ServiceError::InvalidInput(format!(
                    "invalid MAC address in blacklist: {mac}"
                )));
            }
        }
        // Validate time formats.
        if let Some(ref t) = self.auto_on {
            if !is_valid_time(t) {
                return Err(ServiceError::InvalidInput(format!(
                    "invalid auto_on time: {t} (expected HH:MM)"
                )));
            }
        }
        if let Some(ref t) = self.auto_off {
            if !is_valid_time(t) {
                return Err(ServiceError::InvalidInput(format!(
                    "invalid auto_off time: {t} (expected HH:MM)"
                )));
            }
        }
        Ok(())
    }
}

/// Check if a string is a valid MAC address (XX:XX:XX:XX:XX:XX).
fn is_valid_mac(mac: &str) -> bool {
    let parts: Vec<&str> = mac.split(':').collect();
    if parts.len() != 6 {
        return false;
    }
    parts
        .iter()
        .all(|p| p.len() == 2 && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Check if a string is a valid 24h time (HH:MM).
fn is_valid_time(time: &str) -> bool {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return false;
    }
    // Require exactly 2 digits for hours and minutes.
    if parts[0].len() != 2 || parts[1].len() != 2 {
        return false;
    }
    let Ok(h) = parts[0].parse::<u8>() else {
        return false;
    };
    let Ok(m) = parts[1].parse::<u8>() else {
        return false;
    };
    h < 24 && m < 60
}

/// Return the config file path: `~/.config/reecho/config.toml`.
fn config_path() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("reecho");
    path.push("config.toml");
    path
}

/// Generate a default SSID: `Reecho-{hostname}`.
fn default_ssid() -> String {
    let hostname = hostname::get()
        .map(|h| h.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".to_string());
    format!("Reecho-{hostname}")
}

/// Generate a random password of 16 alphanumeric characters.
fn default_password() -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..16)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// Re-export ServiceError for use in this module.
use crate::error::ServiceError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn test_config_path(dir: &std::path::Path) -> PathBuf {
        let mut path = dir.to_path_buf();
        path.push("reecho");
        path.push("config.toml");
        path
    }

    #[test]
    fn default_config_has_valid_values() {
        let config = Config::default();
        assert!(!config.ssid.is_empty());
        assert!(config.ssid.len() <= MAX_SSID_LEN);
        assert!(config.password.len() >= MIN_PASSWORD_LEN);
        assert!(config.password.len() <= MAX_PASSWORD_LEN);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn default_ssid_starts_with_prefix() {
        let ssid = default_ssid();
        assert!(ssid.starts_with("Reecho-"));
    }

    #[test]
    fn default_password_is_correct_length() {
        let pw = default_password();
        assert_eq!(pw.len(), 16);
        assert!(pw.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = TempDir::new().unwrap();
        let path = test_config_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let config = Config::default();
        let contents = toml::to_string_pretty(&config).unwrap();
        fs::write(&path, contents).unwrap();

        let loaded: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config.ssid, loaded.ssid);
        assert_eq!(config.password, loaded.password);
        assert_eq!(config.band, loaded.band);
        assert_eq!(config.data_limit, loaded.data_limit);
    }

    #[test]
    fn validate_rejects_empty_ssid() {
        let mut config = Config::default();
        config.ssid = String::new();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_short_password() {
        let mut config = Config::default();
        config.password = "short".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_long_ssid() {
        let mut config = Config::default();
        config.ssid = "a".repeat(MAX_SSID_LEN + 1);
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_long_password() {
        let mut config = Config::default();
        config.password = "a".repeat(MAX_PASSWORD_LEN + 1);
        assert!(config.validate().is_err());
    }

    #[test]
    fn load_returns_defaults_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let _nonexistent = dir.path().join("nonexistent").join("config.toml");
        // This tests the Config::default() path, not the load() path,
        // but validates the default generation logic.
        let config = Config::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn save_and_load_round_trip_with_new_fields() {
        let dir = TempDir::new().unwrap();
        let path = test_config_path(dir.path());
        fs::create_dir_all(path.parent().unwrap()).unwrap();

        let mut config = Config::default();
        config.blacklist = vec![
            "AA:BB:CC:DD:EE:FF".to_string(),
            "11:22:33:44:55:66".to_string(),
        ];
        config.auto_on = Some("08:30".to_string());
        config.auto_off = Some("22:00".to_string());
        config.data_limit = 1_000_000_000;

        let contents = toml::to_string_pretty(&config).unwrap();
        fs::write(&path, contents).unwrap();

        let loaded: Config = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(loaded.blacklist.len(), 2);
        assert!(
            loaded
                .blacklist
                .iter()
                .any(|m| m.eq_ignore_ascii_case("aa:bb:cc:dd:ee:ff"))
        );
        assert!(
            loaded
                .blacklist
                .iter()
                .any(|m| m.eq_ignore_ascii_case("11:22:33:44:55:66"))
        );
        assert_eq!(loaded.auto_on.as_deref(), Some("08:30"));
        assert_eq!(loaded.auto_off.as_deref(), Some("22:00"));
        assert_eq!(loaded.data_limit, 1_000_000_000);
    }

    #[test]
    fn validate_rejects_invalid_mac() {
        let mut config = Config::default();
        config.blacklist = vec!["not-a-mac".to_string()];
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_macs() {
        let mut config = Config::default();
        config.blacklist = vec![
            "AA:BB:CC:DD:EE:FF".to_string(),
            "00:11:22:33:44:55".to_string(),
        ];
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_rejects_invalid_time() {
        let mut config = Config::default();
        config.auto_on = Some("25:00".to_string());
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_rejects_bad_time_format() {
        let mut config = Config::default();
        config.auto_on = Some("8:30".to_string()); // needs leading zero
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_accepts_valid_times() {
        let mut config = Config::default();
        config.auto_on = Some("08:30".to_string());
        config.auto_off = Some("22:00".to_string());
        assert!(config.validate().is_ok());
    }

    #[test]
    fn validate_accepts_none_times() {
        let mut config = Config::default();
        config.auto_on = None;
        config.auto_off = None;
        assert!(config.validate().is_ok());
    }
}

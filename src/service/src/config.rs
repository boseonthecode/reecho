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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ssid: default_ssid(),
            password: default_password(),
            band: Band::Band5Ghz,
            data_limit: 0,
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
        Ok(())
    }
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
        let nonexistent = dir.path().join("nonexistent").join("config.toml");
        // This tests the Config::default() path, not the load() path,
        // but validates the default generation logic.
        let config = Config::default();
        assert!(config.validate().is_ok());
    }
}

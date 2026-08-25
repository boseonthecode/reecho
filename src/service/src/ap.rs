//! hostapd configuration generation and management.
//!
//! Generates valid hostapd.conf files from hotspot configuration,
//! manages the hostapd subprocess lifecycle, and monitors AP state.

use std::fmt;

use reecho_shared::Band;

use crate::error::ServiceError;

/// hostapd configuration parameters.
#[derive(Debug, Clone)]
pub struct HostapdConfig {
    /// Network interface for the AP (e.g., "wlan0" or "ap0").
    pub interface: String,
    /// Hotspot SSID.
    pub ssid: String,
    /// WPA2/WPA3 passphrase.
    pub password: String,
    /// Wi-Fi band.
    pub band: Band,
    /// Channel number (0 = auto).
    pub channel: u8,
    /// Whether to enable 802.11n (HT).
    pub enable_ht: bool,
    /// Whether to enable 802.11ac (VHT).
    pub enable_vht: bool,
}

impl HostapdConfig {
    /// Create a new hostapd configuration.
    pub fn new(
        interface: String,
        ssid: String,
        password: String,
        band: Band,
    ) -> Result<Self, ServiceError> {
        validate_ssid(&ssid)?;
        validate_password(&password)?;

        Ok(Self {
            interface,
            ssid,
            password,
            band,
            channel: 0, // auto
            enable_ht: true,
            enable_vht: matches!(band, Band::Band5Ghz),
        })
    }

    /// Set the channel explicitly.
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Set 802.11n (HT) mode.
    pub fn with_ht(mut self, enable: bool) -> Self {
        self.enable_ht = enable;
        self
    }

    /// Set 802.11ac (VHT) mode.
    pub fn with_vht(mut self, enable: bool) -> Self {
        self.enable_vht = enable;
        self
    }

    /// Generate the hostapd.conf file contents.
    pub fn to_hostapd_conf(&self) -> String {
        let hw_mode = match self.band {
            Band::Band2_4Ghz => "g",
            Band::Band5Ghz => "a",
        };

        let mut conf = format!(
            r#"interface={interface}
driver=nl80211
ssid={ssid}
hw_mode={hw_mode}
channel={channel}
wmm_enabled=1
auth_algs=1
wpa=2
wpa_passphrase={password}
wpa_key_mgmt=WPA-PSK
rsn_pairwise=CCMP
"#,
            interface = self.interface,
            ssid = self.ssid,
            hw_mode = hw_mode,
            channel = self.channel,
            password = self.password,
        );

        if self.enable_ht {
            conf.push_str("ieee80211n=1\n");
            conf.push_str("ht_capab=[HT40+][SHORT-GI-20][SHORT-GI-40]\n");
        }

        if self.enable_vht && self.band == Band::Band5Ghz {
            conf.push_str("ieee80211ac=1\n");
            conf.push_str("vht_oper_chwidth=1\n");
            conf.push_str("vht_capab=[SHORT-GI-80][SU-BEAMFORMEE]\n");
        }

        conf
    }
}

impl fmt::Display for HostapdConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({} on {})",
            self.ssid,
            self.band,
            self.interface,
        )
    }
}

/// Validate SSID against 802.11 limits.
fn validate_ssid(ssid: &str) -> Result<(), ServiceError> {
    if ssid.is_empty() {
        return Err(ServiceError::InvalidInput(
            "SSID cannot be empty".to_string(),
        ));
    }
    if ssid.len() > reecho_shared::MAX_SSID_LEN {
        return Err(ServiceError::InvalidInput(format!(
            "SSID too long: {} bytes (max {})",
            ssid.len(),
            reecho_shared::MAX_SSID_LEN,
        )));
    }
    // Check for control characters (0x00-0x1F except common whitespace).
    for (i, ch) in ssid.chars().enumerate() {
        if ch.is_control() && ch != ' ' {
            return Err(ServiceError::InvalidInput(format!(
                "SSID contains control character at position {i}"
            )));
        }
    }
    Ok(())
}

/// Validate password against WPA2/WPA3 limits.
fn validate_password(password: &str) -> Result<(), ServiceError> {
    if password.len() < reecho_shared::MIN_PASSWORD_LEN {
        return Err(ServiceError::InvalidInput(format!(
            "Password too short: {} chars (min {})",
            password.len(),
            reecho_shared::MIN_PASSWORD_LEN,
        )));
    }
    if password.len() > reecho_shared::MAX_PASSWORD_LEN {
        return Err(ServiceError::InvalidInput(format!(
            "Password too long: {} chars (max {})",
            password.len(),
            reecho_shared::MAX_PASSWORD_LEN,
        )));
    }
    // WPA2-PSK requires the passphrase to be between 8 and 63 printable ASCII characters.
    // Hex (64-char) passphrases are also allowed but we won't generate them.
    for (i, ch) in password.chars().enumerate() {
        if !ch.is_ascii_graphic() && ch != ' ' {
            return Err(ServiceError::InvalidInput(format!(
                "Password contains non-printable character at position {i}"
            )));
        }
    }
    Ok(())
}

/// Validate a hostapd configuration.
pub fn validate_config(config: &HostapdConfig) -> Result<(), ServiceError> {
    validate_ssid(&config.ssid)?;
    validate_password(&config.password)?;

    if config.interface.is_empty() {
        return Err(ServiceError::InvalidInput(
            "Interface cannot be empty".to_string(),
        ));
    }

    // Validate channel range.
    match config.band {
        Band::Band2_4Ghz => {
            if config.channel > 14 {
                return Err(ServiceError::InvalidInput(format!(
                    "Invalid channel {} for 2.4GHz (must be 0-14)",
                    config.channel,
                )));
            }
        }
        Band::Band5Ghz => {
            if config.channel > 165 {
                return Err(ServiceError::InvalidInput(format!(
                    "Invalid channel {} for 5GHz (must be 0-165)",
                    config.channel,
                )));
            }
        }
    }

    Ok(())
}

// --- Process Management ---

use std::path::PathBuf;
use std::process::Stdio;

/// Errors from hostapd process management.
#[derive(Debug, thiserror::Error)]
pub enum HostapdError {
    /// Failed to write config file.
    #[error("failed to write config: {0}")]
    ConfigWrite(String),

    /// Failed to start hostapd.
    #[error("failed to start hostapd: {0}")]
    StartFailed(String),

    /// hostapd process exited unexpectedly.
    #[error("hostapd exited with code {code}: {stderr}")]
    Exited { code: i32, stderr: String },

    /// hostapd process was killed (e.g., by signal).
    #[error("hostapd was killed: {0}")]
    Killed(String),

    /// Failed to stop hostapd.
    #[error("failed to stop hostapd: {0}")]
    StopFailed(String),

    /// hostapd binary not found.
    #[error("hostapd binary not found at {0}")]
    BinaryNotFound(String),
}

/// Result of starting hostapd.
#[derive(Debug)]
pub struct HostapdProcess {
    /// Path to the config file used.
    pub config_path: PathBuf,
    /// PID of the hostapd process.
    pub pid: u32,
}

/// Trait for spawning hostapd processes, allowing mocking.
#[async_trait::async_trait]
pub trait HostapdSpawner {
    /// Spawn hostapd with the given config file.
    ///
    /// Returns the PID of the spawned process.
    async fn spawn_hostapd(&self, config_path: &PathBuf) -> Result<u32, HostapdError>;

    /// Kill a process by PID.
    async fn kill_process(&self, pid: u32) -> Result<(), HostapdError>;

    /// Check if a process is still running.
    async fn is_running(&self, pid: u32) -> bool;
}

/// Real hostapd spawner using tokio::process.
pub struct RealHostapdSpawner;

#[async_trait::async_trait]
impl HostapdSpawner for RealHostapdSpawner {
    async fn spawn_hostapd(&self, config_path: &PathBuf) -> Result<u32, HostapdError> {
        let child = tokio::process::Command::new("hostapd")
            .arg(config_path.to_str().unwrap_or_default())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HostapdError::BinaryNotFound("hostapd".to_string())
                } else {
                    HostapdError::StartFailed(e.to_string())
                }
            })?;

        let pid = child.id().unwrap_or(0);

        // Detach the child so it runs independently.
        // We'll manage it via hostapd_cli later.
        tokio::spawn(async move {
            let _ = child.wait_with_output().await;
        });

        Ok(pid)
    }

    async fn kill_process(&self, pid: u32) -> Result<(), HostapdError> {
        unsafe {
            libc::kill(pid as i32, libc::SIGTERM);
        }

        // Give it a moment to shut down gracefully.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Force kill if still running.
        if self.is_running(pid).await {
            unsafe {
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }

        Ok(())
    }

    async fn is_running(&self, pid: u32) -> bool {
        // Check if process exists by sending signal 0.
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

/// Write hostapd config to a temporary file and return the path.
pub async fn write_config_file(config: &HostapdConfig) -> Result<PathBuf, HostapdError> {
    let conf_content = config.to_hostapd_conf();
    let dir = std::env::temp_dir();
    let file_name = format!("reecho-hostapd-{}.conf", config.interface);
    let path = dir.join(file_name);

    tokio::fs::write(&path, conf_content)
        .await
        .map_err(|e| HostapdError::ConfigWrite(e.to_string()))?;

    Ok(path)
}

/// Start hostapd with the given configuration.
pub async fn start_hostapd(
    config: &HostapdConfig,
    spawner: &(impl HostapdSpawner + Sync),
) -> Result<HostapdProcess, HostapdError> {
    validate_config(config).map_err(|e| HostapdError::StartFailed(e.to_string()))?;

    let config_path = write_config_file(config).await?;
    let pid = spawner.spawn_hostapd(&config_path).await?;

    tracing::info!("started hostapd (pid {pid}) with config {}", config_path.display());

    Ok(HostapdProcess {
        config_path,
        pid,
    })
}

/// Stop hostapd and clean up config file.
pub async fn stop_hostapd(
    process: &HostapdProcess,
    spawner: &(impl HostapdSpawner + Sync),
) -> Result<(), HostapdError> {
    tracing::info!("stopping hostapd (pid {})", process.pid);

    spawner.kill_process(process.pid).await?;

    // Clean up config file.
    let _ = tokio::fs::remove_file(&process.config_path).await;

    Ok(())
}

/// Mock hostapd spawner for testing.
pub struct MockHostapdSpawner {
    next_pid: std::sync::atomic::AtomicU32,
    running: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
}

impl MockHostapdSpawner {
    pub fn new() -> Self {
        Self {
            next_pid: std::sync::atomic::AtomicU32::new(1000),
            running: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new())),
        }
    }

    pub fn get_running_pids(&self) -> Vec<u32> {
        self.running.lock().unwrap().iter().copied().collect()
    }
}

#[async_trait::async_trait]
impl HostapdSpawner for MockHostapdSpawner {
    async fn spawn_hostapd(&self, _config_path: &PathBuf) -> Result<u32, HostapdError> {
        let pid = self
            .next_pid
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.running.lock().unwrap().insert(pid);
        Ok(pid)
    }

    async fn kill_process(&self, pid: u32) -> Result<(), HostapdError> {
        self.running.lock().unwrap().remove(&pid);
        Ok(())
    }

    async fn is_running(&self, pid: u32) -> bool {
        self.running.lock().unwrap().contains(&pid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_conf_2_4ghz() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band2_4Ghz,
        )
        .unwrap();

        let conf = config.to_hostapd_conf();
        assert!(conf.contains("interface=wlan0"));
        assert!(conf.contains("ssid=TestNet"));
        assert!(conf.contains("hw_mode=g"));
        assert!(conf.contains("wpa_passphrase=password123"));
        assert!(conf.contains("ieee80211n=1"));
        assert!(!conf.contains("ieee80211ac=1"));
    }

    #[test]
    fn generate_conf_5ghz() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "FastNet".to_string(),
            "securepass1".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();

        let conf = config.to_hostapd_conf();
        assert!(conf.contains("hw_mode=a"));
        assert!(conf.contains("ieee80211n=1"));
        assert!(conf.contains("ieee80211ac=1"));
        assert!(conf.contains("vht_oper_chwidth=1"));
    }

    #[test]
    fn generate_conf_with_channel() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band2_4Ghz,
        )
        .unwrap()
        .with_channel(6);

        let conf = config.to_hostapd_conf();
        assert!(conf.contains("channel=6"));
    }

    #[test]
    fn generate_conf_auto_channel() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band2_4Ghz,
        )
        .unwrap();

        let conf = config.to_hostapd_conf();
        assert!(conf.contains("channel=0"));
    }

    #[test]
    fn generate_conf_ht_disabled() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band2_4Ghz,
        )
        .unwrap()
        .with_ht(false);

        let conf = config.to_hostapd_conf();
        assert!(!conf.contains("ieee80211n=1"));
        assert!(!conf.contains("ht_capab"));
    }

    #[test]
    fn validate_config_valid() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn validate_config_empty_interface() {
        let config = HostapdConfig {
            interface: String::new(),
            ssid: "Test".to_string(),
            password: "password123".to_string(),
            band: Band::Band5Ghz,
            channel: 0,
            enable_ht: true,
            enable_vht: false,
        };
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_invalid_channel_2_4ghz() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "Test".to_string(),
            "password123".to_string(),
            Band::Band2_4Ghz,
        )
        .unwrap()
        .with_channel(20);
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn validate_config_invalid_channel_5ghz() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "Test".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap()
        .with_channel(200);
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn reject_empty_ssid() {
        let result = HostapdConfig::new(
            "wlan0".to_string(),
            String::new(),
            "password123".to_string(),
            Band::Band5Ghz,
        );
        assert!(result.is_err());
    }

    #[test]
    fn reject_short_password() {
        let result = HostapdConfig::new(
            "wlan0".to_string(),
            "Test".to_string(),
            "short".to_string(),
            Band::Band5Ghz,
        );
        assert!(result.is_err());
    }

    #[test]
    fn reject_long_password() {
        let result = HostapdConfig::new(
            "wlan0".to_string(),
            "Test".to_string(),
            "a".repeat(64).to_string(),
            Band::Band5Ghz,
        );
        assert!(result.is_err());
    }

    #[test]
    fn reject_control_chars_in_ssid() {
        let result = HostapdConfig::new(
            "wlan0".to_string(),
            "Test\x00Net".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        );
        assert!(result.is_err());
    }

    #[test]
    fn allow_space_in_ssid() {
        let result = HostapdConfig::new(
            "wlan0".to_string(),
            "My Network".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn hostapd_config_display() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();
        assert_eq!(config.to_string(), "TestNet (5GHz on wlan0)");
    }

    #[test]
    fn generate_conf_has_all_required_fields() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();

        let conf = config.to_hostapd_conf();
        assert!(conf.contains("driver=nl80211"));
        assert!(conf.contains("wmm_enabled=1"));
        assert!(conf.contains("auth_algs=1"));
        assert!(conf.contains("wpa=2"));
        assert!(conf.contains("wpa_key_mgmt=WPA-PSK"));
        assert!(conf.contains("rsn_pairwise=CCMP"));
    }

    // --- Process management tests ---

    #[tokio::test]
    async fn start_hostapd_with_mock() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();

        let spawner = MockHostapdSpawner::new();
        let process = start_hostapd(&config, &spawner).await.unwrap();

        assert!(process.pid > 0);
        assert!(spawner.is_running(process.pid).await);

        // Clean up temp config file.
        let _ = tokio::fs::remove_file(&process.config_path).await;
    }

    #[tokio::test]
    async fn stop_hostapd_with_mock() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();

        let spawner = MockHostapdSpawner::new();
        let process = start_hostapd(&config, &spawner).await.unwrap();
        assert!(spawner.is_running(process.pid).await);

        stop_hostapd(&process, &spawner).await.unwrap();
        assert!(!spawner.is_running(process.pid).await);
    }

    #[tokio::test]
    async fn write_config_file_creates_file() {
        let config = HostapdConfig::new(
            "wlan0".to_string(),
            "TestNet".to_string(),
            "password123".to_string(),
            Band::Band5Ghz,
        )
        .unwrap();

        let path = write_config_file(&config).await.unwrap();
        assert!(path.exists());

        let contents = tokio::fs::read_to_string(&path).await.unwrap();
        assert!(contents.contains("ssid=TestNet"));
        assert!(contents.contains("interface=wlan0"));

        let _ = tokio::fs::remove_file(&path).await;
    }

    #[test]
    fn mock_spawner_pid_increments() {
        let spawner = MockHostapdSpawner::new();
        let pid1 = spawner
            .next_pid
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let pid2 = spawner
            .next_pid
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        assert!(pid2 > pid1);
    }
}

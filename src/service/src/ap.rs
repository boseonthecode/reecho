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
        write!(f, "{} ({} on {})", self.ssid, self.band, self.interface,)
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

    tracing::info!(
        "started hostapd (pid {pid}) with config {}",
        config_path.display()
    );

    Ok(HostapdProcess { config_path, pid })
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

// --- Lifecycle Monitoring ---

use std::collections::HashMap;

/// AP state for lifecycle tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApState {
    /// AP is not running.
    Down,
    /// AP is starting up.
    Starting,
    /// AP is fully up and accepting connections.
    Up,
    /// AP crashed or stopped unexpectedly.
    Failed,
}

impl std::fmt::Display for ApState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Down => write!(f, "down"),
            Self::Starting => write!(f, "starting"),
            Self::Up => write!(f, "up"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

/// Result from `hostapd_cli status` parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostapdStatus {
    /// Whether hostapd is running.
    pub is_running: bool,
    /// Whether the AP interface is up.
    pub ap_is_up: bool,
    /// Number of connected stations.
    pub station_count: u32,
    /// The SSID being broadcast.
    pub ssid: String,
    /// Raw key-value pairs from hostapd_cli status.
    pub fields: HashMap<String, String>,
}

impl Default for HostapdStatus {
    fn default() -> Self {
        Self {
            is_running: false,
            ap_is_up: false,
            station_count: 0,
            ssid: String::new(),
            fields: HashMap::new(),
        }
    }
}

/// Parse `hostapd_cli status` output into structured data.
///
/// Example output:
/// ```text
/// state=ENABLED
/// freq=2437
/// channel=6
/// ssid=TestNetwork
/// num_sta[0]=2
/// ```
pub fn parse_hostapd_status(output: &str) -> HostapdStatus {
    let mut fields = HashMap::new();
    let mut ssid = String::new();
    let mut station_count: u32 = 0;

    for line in output.lines() {
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim().to_string();
            let value = value.trim().to_string();
            fields.insert(key.clone(), value.clone());

            if key == "ssid" {
                ssid = value.clone();
            }
            // Count stations from num_sta[N] fields.
            if key.starts_with("num_sta[") {
                if let Ok(count) = value.parse::<u32>() {
                    station_count += count;
                }
            }
        }
    }

    let state = fields.get("state").map(|s| s.as_str()).unwrap_or("");
    let ap_is_up = state == "ENABLED" || state == "ACTIVE";

    HostapdStatus {
        is_running: !fields.is_empty(),
        ap_is_up,
        station_count,
        ssid,
        fields,
    }
}

/// Trait for querying hostapd status, allowing mocking.
#[async_trait::async_trait]
pub trait HostapdStatusQuerier {
    /// Query hostapd_cli status for the given interface.
    async fn query_status(&self, interface: &str) -> Result<String, HostapdError>;

    /// Check if the hostapd process is still running.
    async fn is_process_running(&self, pid: u32) -> bool;
}

/// Real hostapd status querier using hostapd_cli.
pub struct RealHostapdStatusQuerier;

#[async_trait::async_trait]
impl HostapdStatusQuerier for RealHostapdStatusQuerier {
    async fn query_status(&self, interface: &str) -> Result<String, HostapdError> {
        let output = tokio::process::Command::new("hostapd_cli")
            .args(["-i", interface, "status"])
            .output()
            .await
            .map_err(|e| HostapdError::StopFailed(format!("hostapd_cli failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(HostapdError::StopFailed(format!(
                "hostapd_cli status failed: {stderr}"
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| HostapdError::StopFailed(format!("invalid UTF-8: {e}")))
    }

    async fn is_process_running(&self, pid: u32) -> bool {
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }
}

/// Lifecycle monitor that tracks AP state changes.
pub struct ApLifecycleMonitor {
    state: ApState,
    interface: String,
}

impl ApLifecycleMonitor {
    /// Create a new lifecycle monitor for the given interface.
    pub fn new(interface: &str) -> Self {
        Self {
            state: ApState::Down,
            interface: interface.to_string(),
        }
    }

    /// Get the current AP state.
    pub fn state(&self) -> ApState {
        self.state
    }

    /// Update the state based on hostapd_cli status output.
    pub fn update_from_status(&mut self, status: &HostapdStatus) -> Option<ApState> {
        let old_state = self.state;

        if !status.is_running {
            self.state = if old_state == ApState::Starting || old_state == ApState::Up {
                ApState::Failed
            } else {
                ApState::Down
            };
        } else if status.ap_is_up {
            self.state = ApState::Up;
        } else {
            self.state = ApState::Starting;
        }

        if self.state != old_state {
            tracing::info!(
                "AP state changed on {}: {} -> {}",
                self.interface,
                old_state,
                self.state
            );
            Some(self.state)
        } else {
            None
        }
    }

    /// Mark the AP as down.
    pub fn mark_down(&mut self) -> Option<ApState> {
        let old = self.state;
        self.state = ApState::Down;
        if old != ApState::Down {
            Some(self.state)
        } else {
            None
        }
    }

    /// Mark the AP as failed.
    pub fn mark_failed(&mut self) -> Option<ApState> {
        let old = self.state;
        self.state = ApState::Failed;
        if old != ApState::Failed {
            Some(self.state)
        } else {
            None
        }
    }
}

/// Monitor hostapd and return state changes.
///
/// Polls `hostapd_cli status` at the given interval and checks if the process
/// is still running. Returns when the process exits or `stop` is set.
pub async fn monitor_hostapd(
    process: &HostapdProcess,
    querier: &(impl HostapdStatusQuerier + Sync),
    poll_interval_ms: u64,
) -> HostapdStatus {
    let mut monitor = ApLifecycleMonitor::new(&process.pid.to_string());

    loop {
        // Check if process is still running.
        if !querier.is_process_running(process.pid).await {
            monitor.mark_failed();
            tracing::warn!("hostapd process {} exited unexpectedly", process.pid);
            break;
        }

        // Query status via hostapd_cli.
        match querier.query_status(&process.pid.to_string()).await {
            Ok(output) => {
                let status = parse_hostapd_status(&output);
                monitor.update_from_status(&status);

                if monitor.state() == ApState::Up || monitor.state() == ApState::Failed {
                    return status;
                }
            }
            Err(e) => {
                tracing::debug!("hostapd_cli query failed: {e}");
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(poll_interval_ms)).await;
    }

    HostapdStatus::default()
}

/// Mock hostapd status querier for testing.
pub struct MockHostapdStatusQuerier {
    status_sequence: std::sync::Mutex<Vec<String>>,
    call_count: std::sync::atomic::AtomicUsize,
    running_pids: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,
}

impl MockHostapdStatusQuerier {
    pub fn new() -> Self {
        Self {
            status_sequence: std::sync::Mutex::new(Vec::new()),
            call_count: std::sync::atomic::AtomicUsize::new(0),
            running_pids: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashSet::new(),
            )),
        }
    }

    pub fn add_status_response(&self, response: &str) {
        self.status_sequence
            .lock()
            .unwrap()
            .push(response.to_string());
    }

    pub fn set_process_running(&self, pid: u32, running: bool) {
        if running {
            self.running_pids.lock().unwrap().insert(pid);
        } else {
            self.running_pids.lock().unwrap().remove(&pid);
        }
    }
}

#[async_trait::async_trait]
impl HostapdStatusQuerier for MockHostapdStatusQuerier {
    async fn query_status(&self, _interface: &str) -> Result<String, HostapdError> {
        let idx = self
            .call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let sequence = self.status_sequence.lock().unwrap();
        if idx < sequence.len() {
            Ok(sequence[idx].clone())
        } else {
            Ok(String::new())
        }
    }

    async fn is_process_running(&self, pid: u32) -> bool {
        self.running_pids.lock().unwrap().contains(&pid)
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

    // --- Lifecycle monitoring tests ---

    #[test]
    fn parse_hostapd_status_empty() {
        let status = parse_hostapd_status("");
        assert!(!status.is_running);
        assert!(!status.ap_is_up);
        assert_eq!(status.station_count, 0);
    }

    #[test]
    fn parse_hostapd_status_with_ap_up() {
        let output = "state=ENABLED\nfreq=2437\nchannel=6\nssid=TestNet\nnum_sta[0]=2\n";
        let status = parse_hostapd_status(output);
        assert!(status.is_running);
        assert!(status.ap_is_up);
        assert_eq!(status.station_count, 2);
        assert_eq!(status.ssid, "TestNet");
    }

    #[test]
    fn parse_hostapd_status_starting() {
        let output = "state=BEACONING\n";
        let status = parse_hostapd_status(output);
        assert!(status.is_running);
        assert!(!status.ap_is_up);
    }

    #[test]
    fn parse_hostapd_status_counts_multiple_stations() {
        let output = "state=ENABLED\nssid=Test\nnum_sta[0]=3\nnum_sta[1]=1\n";
        let status = parse_hostapd_status(output);
        assert_eq!(status.station_count, 4);
    }

    #[test]
    fn lifecycle_monitor_state_transitions() {
        let mut monitor = ApLifecycleMonitor::new("wlan0");
        assert_eq!(monitor.state(), ApState::Down);

        // Starting -> Up.
        let status = HostapdStatus {
            is_running: true,
            ap_is_up: false,
            station_count: 0,
            ssid: String::new(),
            fields: HashMap::new(),
        };
        let transition = monitor.update_from_status(&status);
        assert_eq!(transition, Some(ApState::Starting));
        assert_eq!(monitor.state(), ApState::Starting);

        // Up.
        let status = HostapdStatus {
            is_running: true,
            ap_is_up: true,
            station_count: 0,
            ssid: "Test".to_string(),
            fields: HashMap::new(),
        };
        let transition = monitor.update_from_status(&status);
        assert_eq!(transition, Some(ApState::Up));
        assert_eq!(monitor.state(), ApState::Up);
    }

    #[test]
    fn lifecycle_monitor_crash_detection() {
        let mut monitor = ApLifecycleMonitor::new("wlan0");

        // Start -> Up.
        let status = HostapdStatus {
            is_running: true,
            ap_is_up: true,
            station_count: 0,
            ssid: "Test".to_string(),
            fields: HashMap::new(),
        };
        monitor.update_from_status(&status);
        assert_eq!(monitor.state(), ApState::Up);

        // Crash -> Failed.
        let status = HostapdStatus {
            is_running: false,
            ap_is_up: false,
            station_count: 0,
            ssid: String::new(),
            fields: HashMap::new(),
        };
        let transition = monitor.update_from_status(&status);
        assert_eq!(transition, Some(ApState::Failed));
    }

    #[test]
    fn lifecycle_monitor_no_change_returns_none() {
        let mut monitor = ApLifecycleMonitor::new("wlan0");
        let status = HostapdStatus {
            is_running: false,
            ap_is_up: false,
            station_count: 0,
            ssid: String::new(),
            fields: HashMap::new(),
        };
        // First call: Down -> Down = no change.
        let transition = monitor.update_from_status(&status);
        assert_eq!(transition, None);
    }

    #[test]
    fn lifecycle_monitor_mark_down() {
        let mut monitor = ApLifecycleMonitor::new("wlan0");
        let status = HostapdStatus {
            is_running: true,
            ap_is_up: true,
            station_count: 0,
            ssid: "Test".to_string(),
            fields: HashMap::new(),
        };
        monitor.update_from_status(&status);
        assert_eq!(monitor.state(), ApState::Up);

        let transition = monitor.mark_down();
        assert_eq!(transition, Some(ApState::Down));
        assert_eq!(monitor.state(), ApState::Down);

        // Mark down again -> no change.
        assert_eq!(monitor.mark_down(), None);
    }

    #[test]
    fn lifecycle_monitor_mark_failed() {
        let mut monitor = ApLifecycleMonitor::new("wlan0");
        let transition = monitor.mark_failed();
        assert_eq!(transition, Some(ApState::Failed));

        // Mark failed again -> no change.
        assert_eq!(monitor.mark_failed(), None);
    }

    #[tokio::test]
    async fn monitor_hostapd_detects_up() {
        let querier = MockHostapdStatusQuerier::new();
        querier.add_status_response("state=ENABLED\nssid=TestNet\n");
        querier.set_process_running(1000, true);

        let process = HostapdProcess {
            config_path: PathBuf::from("/tmp/test.conf"),
            pid: 1000,
        };

        let status = monitor_hostapd(&process, &querier, 10).await;
        assert!(status.ap_is_up);
        assert_eq!(status.ssid, "TestNet");
    }

    #[tokio::test]
    async fn monitor_hostapd_detects_crash() {
        let querier = MockHostapdStatusQuerier::new();
        // Process is not running from the start.
        querier.set_process_running(1000, false);

        let process = HostapdProcess {
            config_path: PathBuf::from("/tmp/test.conf"),
            pid: 1000,
        };

        let status = monitor_hostapd(&process, &querier, 10).await;
        assert!(!status.ap_is_up);
    }
}

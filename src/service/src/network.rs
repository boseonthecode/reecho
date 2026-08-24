//! NetworkManager interaction for Wi-Fi state detection.
//!
//! Queries NetworkManager via D-Bus to detect the active Wi-Fi connection,
//! extract SSID, interface name, signal strength, and connection state.

use std::fmt;

use zbus::Connection;

use reecho_shared::MAX_SSID_LEN;

/// NetworkManager D-Bus bus name.
const NM_NAME: &str = "org.freedesktop.NetworkManager";

/// NetworkManager object path.
const NM_PATH: &str = "/org/freedesktop/NetworkManager";

/// Device type constant for Wi-Fi.
const DEVICE_TYPE_WIFI: u32 = 2;

/// Connection state: activated.
const NM_ACTIVE_STATE_ACTIVATED: u32 = 2;

/// Information about the current Wi-Fi connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WifiState {
    /// The SSID of the connected network.
    pub ssid: String,
    /// The network interface name (e.g., "wlan0").
    pub interface: String,
    /// Signal strength in percentage (0–100).
    pub signal_strength: u8,
    /// Whether the Wi-Fi connection is active.
    pub is_connected: bool,
    /// The object path of the active connection.
    pub active_connection_path: String,
}

impl fmt::Display for WifiState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_connected {
            write!(
                f,
                "{} ({}, {}%)",
                self.ssid, self.interface, self.signal_strength
            )
        } else {
            write!(f, "disconnected")
        }
    }
}

/// Trait for NetworkManager operations, allowing mocking in tests.
#[async_trait::async_trait]
pub trait NetworkManagerOps {
    /// Get the object paths of all network devices.
    async fn get_devices(&self) -> Result<Vec<String>, NetworkError>;

    /// Get the device type for a given device path.
    async fn get_device_type(&self, device_path: &str) -> Result<u32, NetworkError>;

    /// Get the interface name for a given device path.
    async fn get_device_interface(&self, device_path: &str) -> Result<String, NetworkError>;

    /// Get the active connection paths.
    async fn get_active_connections(&self) -> Result<Vec<String>, NetworkError>;

    /// Get the connection state for an active connection.
    async fn get_active_connection_state(
        &self,
        active_path: &str,
    ) -> Result<u32, NetworkError>;

    /// Get the device object path for an active connection.
    async fn get_active_connection_device(
        &self,
        active_path: &str,
    ) -> Result<String, NetworkError>;

    /// Get the access point paths for a wireless device.
    async fn get_access_points(&self, device_path: &str) -> Result<Vec<String>, NetworkError>;

    /// Get the SSID from an access point path.
    async fn get_ap_ssid(&self, ap_path: &str) -> Result<Vec<u8>, NetworkError>;

    /// Get the signal strength from an access point path.
    async fn get_ap_strength(&self, ap_path: &str) -> Result<u8, NetworkError>;
}

/// Errors during NetworkManager interaction.
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    /// D-Bus communication error.
    #[error("D-Bus error: {0}")]
    Dbus(String),

    /// No active Wi-Fi connection found.
    #[error("no active Wi-Fi connection")]
    NoActiveWifi,

    /// Invalid SSID (too long or empty).
    #[error("invalid SSID: {0}")]
    InvalidSsid(String),

    /// Failed to parse D-Bus response.
    #[error("parse error: {0}")]
    Parse(String),
}

/// Real NetworkManager client using D-Bus.
pub struct NetworkManagerClient {
    connection: Connection,
}

impl NetworkManagerClient {
    /// Create a new client connected to the system bus.
    pub async fn new() -> Result<Self, NetworkError> {
        let connection = Connection::system()
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;
        Ok(Self { connection })
    }

    /// Create a client from an existing connection.
    pub fn from_connection(connection: Connection) -> Self {
        Self { connection }
    }
}

#[async_trait::async_trait]
impl NetworkManagerOps for NetworkManagerClient {
    async fn get_devices(&self) -> Result<Vec<String>, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            NM_PATH,
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let devices: Vec<String> = proxy
            .call_method("GetDevices", &())
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(devices)
    }

    async fn get_device_type(&self, device_path: &str) -> Result<u32, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            device_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let device_type: u32 = proxy
            .call_method(
                "Get",
                &("org.freedesktop.NetworkManager.Device", "DeviceType"),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(device_type)
    }

    async fn get_device_interface(&self, device_path: &str) -> Result<String, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            device_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let interface: String = proxy
            .call_method(
                "Get",
                &("org.freedesktop.NetworkManager.Device", "Interface"),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(interface)
    }

    async fn get_active_connections(&self) -> Result<Vec<String>, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            NM_PATH,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let active_conns: Vec<String> = proxy
            .call_method(
                "Get",
                &("org.freedesktop.NetworkManager", "ActiveConnections"),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(active_conns)
    }

    async fn get_active_connection_state(
        &self,
        active_path: &str,
    ) -> Result<u32, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            active_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let state: u32 = proxy
            .call_method(
                "Get",
                &(
                    "org.freedesktop.NetworkManager.Connection.Active",
                    "State",
                ),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(state)
    }

    async fn get_active_connection_device(
        &self,
        active_path: &str,
    ) -> Result<String, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            active_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let device: String = proxy
            .call_method(
                "Get",
                &(
                    "org.freedesktop.NetworkManager.Connection.Active",
                    "Devices",
                ),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(device)
    }

    async fn get_access_points(&self, device_path: &str) -> Result<Vec<String>, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            device_path,
            "org.freedesktop.NetworkManager.Device.Wireless",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let access_points: Vec<String> = proxy
            .call_method("GetAccessPoints", &())
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(access_points)
    }

    async fn get_ap_ssid(&self, ap_path: &str) -> Result<Vec<u8>, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            ap_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let ssid: Vec<u8> = proxy
            .call_method(
                "Get",
                &("org.freedesktop.NetworkManager.AccessPoint", "Ssid"),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(ssid)
    }

    async fn get_ap_strength(&self, ap_path: &str) -> Result<u8, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            ap_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let strength: u8 = proxy
            .call_method(
                "Get",
                &(
                    "org.freedesktop.NetworkManager.AccessPoint",
                    "Strength",
                ),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(strength)
    }
}

/// Detect the current Wi-Fi state from NetworkManager.
pub async fn get_wifi_state(
    nm: &(impl NetworkManagerOps + Sync),
) -> Result<WifiState, NetworkError> {
    let devices = nm.get_devices().await?;

    // Find the first Wi-Fi device.
    let mut wifi_device: Option<String> = None;
    for device_path in &devices {
        let device_type = nm.get_device_type(device_path).await?;
        if device_type == DEVICE_TYPE_WIFI {
            wifi_device = Some(device_path.clone());
            break;
        }
    }

    let wifi_device = wifi_device.ok_or(NetworkError::NoActiveWifi)?;
    let interface = nm.get_device_interface(&wifi_device).await?;

    // Find an active connection on this device.
    let active_connections = nm.get_active_connections().await?;
    let mut ssid = String::new();
    let mut signal_strength: u8 = 0;
    let mut is_connected = false;
    let mut active_connection_path = String::new();

    for active_path in &active_connections {
        let state = nm.get_active_connection_state(active_path).await?;
        if state == NM_ACTIVE_STATE_ACTIVATED {
            active_connection_path = active_path.clone();
            is_connected = true;

            // Try to get the SSID from access points.
            let access_points = nm.get_access_points(&wifi_device).await?;
            for ap_path in &access_points {
                let ap_ssid = nm.get_ap_ssid(ap_path).await?;
                let strength = nm.get_ap_strength(ap_path).await?;
                if !ap_ssid.is_empty() {
                    ssid = String::from_utf8_lossy(&ap_ssid).into_owned();
                    signal_strength = strength;
                    break;
                }
            }
            break;
        }
    }

    // Validate SSID length.
    if !ssid.is_empty() && ssid.len() > MAX_SSID_LEN {
        return Err(NetworkError::InvalidSsid(format!(
            "SSID too long: {} bytes (max {})",
            ssid.len(),
            MAX_SSID_LEN
        )));
    }

    Ok(WifiState {
        ssid,
        interface,
        signal_strength,
        is_connected,
        active_connection_path,
    })
}

// --- AP+STA Capability Detection ---

/// Result of an AP+STA capability check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApStaCapability {
    /// Whether AP+STA concurrent mode is supported.
    pub supported: bool,
    /// The chipset or driver name, if detectable.
    pub chipset: Option<String>,
    /// Human-readable explanation of the verdict.
    pub reason: String,
}

/// Trait for executing system commands, allowing mocking in tests.
#[async_trait::async_trait]
pub trait CommandRunner {
    /// Run a command and return its stdout.
    async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, NetworkError>;
}

/// Real command runner using `tokio::process::Command`.
pub struct RealCommandRunner;

#[async_trait::async_trait]
impl CommandRunner for RealCommandRunner {
    async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, NetworkError> {
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| NetworkError::Parse(format!("failed to run {command}: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NetworkError::Parse(format!(
                "{command} failed: {stderr}"
            )));
        }

        String::from_utf8(output.stdout)
            .map_err(|e| NetworkError::Parse(format!("invalid UTF-8 from {command}: {e}")))
    }
}

/// Parse `iw list` output to determine AP+STA concurrent mode support.
///
/// Looks for valid interface combinations that include both `AP` and `managed`
/// (STA) modes simultaneously.
pub fn parse_iw_list(output: &str) -> ApStaCapability {
    let mut chipset: Option<String> = None;
    let mut in_combinations = false;
    let mut current_combination = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Extract chipset/driver name from "wiphy <name>" line.
        if trimmed.starts_with("Wiphy ") {
            chipset = Some(trimmed["Wiphy ".len()..].to_string());
        }

        // Detect the start of interface combinations section.
        if trimmed.contains("valid interface combinations:") {
            in_combinations = true;
            continue;
        }

        // Parse combinations.
        if in_combinations {
            if trimmed.starts_with("* {") {
                // Start of a new combination — check the previous one.
                if !current_combination.is_empty() {
                    if has_ap_and_managed(&current_combination) {
                        return ApStaCapability {
                            supported: true,
                            chipset,
                            reason: "Found valid AP+STA combination".to_string(),
                        };
                    }
                }
                current_combination = vec![trimmed.to_string()];
            } else if !trimmed.is_empty() && !trimmed.starts_with("#") {
                current_combination.push(trimmed.to_string());
            } else if trimmed.is_empty() || trimmed.starts_with("Software interface") {
                // End of combinations section.
                if !current_combination.is_empty() && has_ap_and_managed(&current_combination) {
                    return ApStaCapability {
                        supported: true,
                        chipset,
                        reason: "Found valid AP+STA combination".to_string(),
                    };
                }
                in_combinations = false;
            }
        }
    }

    // Check the last combination if we're still in the section.
    if in_combinations && !current_combination.is_empty() {
        if has_ap_and_managed(&current_combination) {
            return ApStaCapability {
                supported: true,
                chipset,
                reason: "Found valid AP+STA combination".to_string(),
            };
        }
    }

    ApStaCapability {
        supported: false,
        chipset,
        reason: "No valid AP+STA combination found in `iw list` output".to_string(),
    }
}

/// Check if a combination of interface modes includes both AP and managed (STA).
fn has_ap_and_managed(combination: &[String]) -> bool {
    let joined = combination.join(" ");
    joined.contains("AP") && (joined.contains("managed") || joined.contains("* managed"))
}

/// Check AP+STA capability by running `iw list`.
pub async fn check_ap_sta_capability(
    runner: &(impl CommandRunner + Sync),
) -> Result<ApStaCapability, NetworkError> {
    let output = runner.run_command("iw", &["list"]).await?;
    Ok(parse_iw_list(&output))
}

/// Mock CommandRunner for testing.
pub struct MockCommandRunner {
    outputs: std::collections::HashMap<String, String>,
}

impl MockCommandRunner {
    pub fn new() -> Self {
        Self {
            outputs: std::collections::HashMap::new(),
        }
    }

    pub fn add_output(&mut self, key: &str, output: &str) {
        self.outputs
            .insert(key.to_string(), output.to_string());
    }
}

#[async_trait::async_trait]
impl CommandRunner for MockCommandRunner {
    async fn run_command(&self, command: &str, _args: &[&str]) -> Result<String, NetworkError> {
        self.outputs
            .get(command)
            .cloned()
            .ok_or_else(|| NetworkError::Parse(format!("no mock output for {command}")))
    }
}

/// Mock NetworkManager for testing.
pub struct MockNetworkManager {
    devices: Vec<String>,
    device_types: std::collections::HashMap<String, u32>,
    device_interfaces: std::collections::HashMap<String, String>,
    active_connections: Vec<String>,
    connection_states: std::collections::HashMap<String, u32>,
    access_points: std::collections::HashMap<String, Vec<String>>,
    ap_ssids: std::collections::HashMap<String, Vec<u8>>,
    ap_strengths: std::collections::HashMap<String, u8>,
}

impl MockNetworkManager {
    pub fn new() -> Self {
        Self {
            devices: Vec::new(),
            device_types: std::collections::HashMap::new(),
            device_interfaces: std::collections::HashMap::new(),
            active_connections: Vec::new(),
            connection_states: std::collections::HashMap::new(),
            access_points: std::collections::HashMap::new(),
            ap_ssids: std::collections::HashMap::new(),
            ap_strengths: std::collections::HashMap::new(),
        }
    }

    pub fn add_wifi_device(&mut self, path: &str, interface: &str) {
        self.devices.push(path.to_string());
        self.device_types
            .insert(path.to_string(), DEVICE_TYPE_WIFI);
        self.device_interfaces
            .insert(path.to_string(), interface.to_string());
    }

    pub fn add_active_connection(&mut self, path: &str, state: u32, device_path: &str) {
        self.active_connections.push(path.to_string());
        self.connection_states.insert(path.to_string(), state);
        self.access_points
            .entry(device_path.to_string())
            .or_default();
    }

    pub fn add_access_point(
        &mut self,
        device_path: &str,
        ap_path: &str,
        ssid: &str,
        strength: u8,
    ) {
        self.access_points
            .entry(device_path.to_string())
            .or_default()
            .push(ap_path.to_string());
        self.ap_ssids
            .insert(ap_path.to_string(), ssid.as_bytes().to_vec());
        self.ap_strengths
            .insert(ap_path.to_string(), strength);
    }
}

#[async_trait::async_trait]
impl NetworkManagerOps for MockNetworkManager {
    async fn get_devices(&self) -> Result<Vec<String>, NetworkError> {
        Ok(self.devices.clone())
    }

    async fn get_device_type(&self, device_path: &str) -> Result<u32, NetworkError> {
        self.device_types
            .get(device_path)
            .copied()
            .ok_or_else(|| NetworkError::Parse(format!("unknown device: {device_path}")))
    }

    async fn get_device_interface(&self, device_path: &str) -> Result<String, NetworkError> {
        self.device_interfaces
            .get(device_path)
            .cloned()
            .ok_or_else(|| NetworkError::Parse(format!("unknown device: {device_path}")))
    }

    async fn get_active_connections(&self) -> Result<Vec<String>, NetworkError> {
        Ok(self.active_connections.clone())
    }

    async fn get_active_connection_state(
        &self,
        active_path: &str,
    ) -> Result<u32, NetworkError> {
        self.connection_states
            .get(active_path)
            .copied()
            .ok_or_else(|| {
                NetworkError::Parse(format!("unknown active connection: {active_path}"))
            })
    }

    async fn get_active_connection_device(
        &self,
        _active_path: &str,
    ) -> Result<String, NetworkError> {
        self.devices
            .first()
            .cloned()
            .ok_or_else(|| NetworkError::Parse("no devices".into()))
    }

    async fn get_access_points(&self, device_path: &str) -> Result<Vec<String>, NetworkError> {
        Ok(self
            .access_points
            .get(device_path)
            .cloned()
            .unwrap_or_default())
    }

    async fn get_ap_ssid(&self, ap_path: &str) -> Result<Vec<u8>, NetworkError> {
        self.ap_ssids
            .get(ap_path)
            .cloned()
            .ok_or_else(|| NetworkError::Parse(format!("unknown AP: {ap_path}")))
    }

    async fn get_ap_strength(&self, ap_path: &str) -> Result<u8, NetworkError> {
        self.ap_strengths
            .get(ap_path)
            .copied()
            .ok_or_else(|| NetworkError::Parse(format!("unknown AP: {ap_path}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn get_wifi_state_returns_connected_wifi() {
        let mut mock = MockNetworkManager::new();
        mock.add_wifi_device("/org/freedesktop/NetworkManager/Devices/0", "wlan0");
        mock.add_active_connection(
            "/org/freedesktop/NetworkManager/ActiveConnection/0",
            NM_ACTIVE_STATE_ACTIVATED,
            "/org/freedesktop/NetworkManager/Devices/0",
        );
        mock.add_access_point(
            "/org/freedesktop/NetworkManager/Devices/0",
            "/org/freedesktop/NetworkManager/AccessPoints/0",
            "TestNetwork",
            85,
        );

        let state = get_wifi_state(&mock).await.unwrap();
        assert!(state.is_connected);
        assert_eq!(state.ssid, "TestNetwork");
        assert_eq!(state.interface, "wlan0");
        assert_eq!(state.signal_strength, 85);
    }

    #[tokio::test]
    async fn get_wifi_state_no_wifi_device() {
        let mock = MockNetworkManager::new();
        let result = get_wifi_state(&mock).await;
        assert!(result.is_err());
        assert!(matches!(result, Err(NetworkError::NoActiveWifi)));
    }

    #[tokio::test]
    async fn get_wifi_state_no_active_connection() {
        let mut mock = MockNetworkManager::new();
        mock.add_wifi_device("/org/freedesktop/NetworkManager/Devices/0", "wlan0");

        let state = get_wifi_state(&mock).await.unwrap();
        assert!(!state.is_connected);
        assert_eq!(state.interface, "wlan0");
        assert!(state.ssid.is_empty());
    }

    #[tokio::test]
    async fn get_wifi_state_connection_not_activated() {
        let mut mock = MockNetworkManager::new();
        mock.add_wifi_device("/org/freedesktop/NetworkManager/Devices/0", "wlan0");
        mock.add_active_connection(
            "/org/freedesktop/NetworkManager/ActiveConnection/0",
            1, // connecting, not activated
            "/org/freedesktop/NetworkManager/Devices/0",
        );

        let state = get_wifi_state(&mock).await.unwrap();
        assert!(!state.is_connected);
    }

    #[tokio::test]
    async fn wifi_state_display_connected() {
        let state = WifiState {
            ssid: "MyNetwork".to_string(),
            interface: "wlan0".to_string(),
            signal_strength: 72,
            is_connected: true,
            active_connection_path: "/test".to_string(),
        };
        assert_eq!(state.to_string(), "MyNetwork (wlan0, 72%)");
    }

    #[tokio::test]
    async fn wifi_state_display_disconnected() {
        let state = WifiState {
            ssid: String::new(),
            interface: "wlan0".to_string(),
            signal_strength: 0,
            is_connected: false,
            active_connection_path: String::new(),
        };
        assert_eq!(state.to_string(), "disconnected");
    }

    // --- AP+STA capability tests ---

    #[test]
    fn parse_iw_list_with_ap_sta_support() {
        let iw_output = r#"Wiphy phy0
        max # scan SSIDs: 4
        max # scan IEs: 2281 bytes
        max # sched scan SSIDs: 0
        max # sched scan match IEs: 0
        max # scheduled scans: 0
        max # match sets: 0
        Retry short limit: 7
        Retry long limit: 4
        Coverage class: 0 (up to 0m)
        Device supports T-DLS.
        Supported interface modes:
         * IBSS
         * managed
         * AP
         * P2P-client
         * P2P-GO
         * P2P device
        valid interface combinations:
         * #{ AP, P2P-device } <= 1, #{ managed } <= 16, total <= 17, DMI要求 <= 1
         * #{ managed } <= 16
        Supported commands:
         * new_interface
         * set_interface
        "#;

        let result = parse_iw_list(iw_output);
        assert!(result.supported);
        assert_eq!(result.chipset, Some("phy0".to_string()));
    }

    #[test]
    fn parse_iw_list_without_ap_sta_support() {
        let iw_output_no_ap = r#"Wiphy phy0
        Supported interface modes:
         * managed
        valid interface combinations:
         * #{ managed } <= 1
        "#;

        let result = parse_iw_list(iw_output_no_ap);
        assert!(!result.supported);
        assert_eq!(result.chipset, Some("phy0".to_string()));
    }

    #[test]
    fn parse_iw_list_empty_output() {
        let result = parse_iw_list("");
        assert!(!result.supported);
        assert!(result.chipset.is_none());
    }

    #[test]
    fn parse_iw_list_extracts_chipset() {
        let iw_output = r#"Wiphy phy1
        Supported interface modes:
         * managed
        valid interface combinations:
         * #{ managed } <= 1
        "#;
        let result = parse_iw_list(iw_output);
        assert_eq!(result.chipset, Some("phy1".to_string()));
    }

    #[tokio::test]
    async fn check_ap_sta_capability_with_mock() {
        let mut mock = MockCommandRunner::new();
        mock.add_output("iw", "Wiphy phy0\nvalid interface combinations:\n * { AP }, #{ managed } <= 1, total <= 2\n");

        let result = check_ap_sta_capability(&mock).await.unwrap();
        assert!(result.supported);
        assert_eq!(result.chipset, Some("phy0".to_string()));
    }

    #[tokio::test]
    async fn check_ap_sta_capability_not_supported() {
        let mut mock = MockCommandRunner::new();
        mock.add_output("iw", "Wiphy phy0\nvalid interface combinations:\n * #{ managed } <= 1\n");

        let result = check_ap_sta_capability(&mock).await.unwrap();
        assert!(!result.supported);
    }
}

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
    async fn get_active_connection_state(&self, active_path: &str) -> Result<u32, NetworkError>;

    /// Get the device object path for an active connection.
    async fn get_active_connection_device(&self, active_path: &str)
    -> Result<String, NetworkError>;

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

    async fn get_active_connection_state(&self, active_path: &str) -> Result<u32, NetworkError> {
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
                &("org.freedesktop.NetworkManager.Connection.Active", "State"),
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
                &("org.freedesktop.NetworkManager.AccessPoint", "Strength"),
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
    nm: &(impl NetworkManagerOps + Sync + ?Sized),
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
            return Err(NetworkError::Parse(format!("{command} failed: {stderr}")));
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
    runner: &(impl CommandRunner + Sync + ?Sized),
) -> Result<ApStaCapability, NetworkError> {
    let output = runner.run_command("iw", &["list"]).await?;
    Ok(parse_iw_list(&output))
}

// --- AP+STA Mode Configuration ---

/// Result of attempting to configure AP+STA mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApStaResult {
    /// Whether the AP interface was successfully created.
    pub success: bool,
    /// The name of the AP interface (e.g., "ap0" or "wlan0_1").
    pub ap_interface: Option<String>,
    /// Whether force mode was used (AP+STA not natively supported).
    pub forced: bool,
    /// Warning message if force mode was used or if there were issues.
    pub warning: Option<String>,
}

/// Connection settings represented as a nested string map.
/// Outer key: setting section (e.g., "connection", "802-11-wireless").
/// Inner key: setting name, value: setting value.
pub type ConnectionSettings =
    std::collections::HashMap<String, std::collections::HashMap<String, String>>;

/// Trait for NetworkManager connection operations, allowing mocking.
#[async_trait::async_trait]
pub trait NetworkManagerConnectionOps {
    /// Add and activate a new connection via NetworkManager.
    ///
    /// Returns the active connection path.
    async fn add_and_activate_connection(
        &self,
        connection_settings: &ConnectionSettings,
        device_path: &str,
    ) -> Result<String, NetworkError>;

    /// Deactivate a connection.
    async fn deactivate_connection(&self, active_connection_path: &str)
    -> Result<(), NetworkError>;

    /// Delete a connection profile.
    async fn delete_connection(&self, connection_path: &str) -> Result<(), NetworkError>;

    /// Get the connection profile path from an active connection.
    async fn get_connection_profile(
        &self,
        active_connection_path: &str,
    ) -> Result<String, NetworkError>;
}

/// Build NM connection settings for an AP interface.
pub fn build_ap_connection_settings(
    ssid: &str,
    password: &str,
    band: &str,
    _interface: &str,
) -> ConnectionSettings {
    let mut settings = std::collections::HashMap::new();

    let mut connection = std::collections::HashMap::new();
    connection.insert("id".to_string(), format!("Reecho-{ssid}"));
    connection.insert("type".to_string(), "802-11-wireless".to_string());
    settings.insert("connection".to_string(), connection);

    let mut wireless = std::collections::HashMap::new();
    wireless.insert("ssid".to_string(), ssid.to_string());
    wireless.insert("mode".to_string(), "ap".to_string());
    wireless.insert("band".to_string(), band.to_string());
    settings.insert("802-11-wireless".to_string(), wireless);

    let mut security = std::collections::HashMap::new();
    security.insert("key-mgmt".to_string(), "wpa-psk".to_string());
    security.insert("psk".to_string(), password.to_string());
    settings.insert("802-11-wireless-security".to_string(), security);

    let mut ipv4 = std::collections::HashMap::new();
    ipv4.insert("method".to_string(), "shared".to_string());
    settings.insert("ipv4".to_string(), ipv4);

    let mut ipv6 = std::collections::HashMap::new();
    ipv6.insert("method".to_string(), "auto".to_string());
    settings.insert("ipv6".to_string(), ipv6);

    settings
}

/// Activate AP+STA mode via NetworkManager.
///
/// Creates a virtual AP interface while preserving the existing STA connection.
/// If AP+STA is not natively supported, proceeds with a warning (force mode).
pub async fn activate_ap_sta(
    nm: &(impl NetworkManagerConnectionOps + Sync + ?Sized),
    ssid: &str,
    password: &str,
    band: &str,
    sta_interface: &str,
    capability: &ApStaCapability,
) -> Result<ApStaResult, NetworkError> {
    let mut warning = None;

    if !capability.supported {
        warning = Some(format!(
            "AP+STA not supported on {}. Proceeding in force mode. Chipset: {}. Reason: {}",
            sta_interface,
            capability.chipset.as_deref().unwrap_or("unknown"),
            capability.reason
        ));
        tracing::warn!("{}", warning.as_ref().unwrap());
    }

    let settings = build_ap_connection_settings(ssid, password, band, sta_interface);

    let _active_path = nm
        .add_and_activate_connection(&settings, sta_interface)
        .await?;

    let ap_interface = Some(sta_interface.to_string());

    Ok(ApStaResult {
        success: true,
        ap_interface,
        forced: !capability.supported,
        warning,
    })
}

/// Deactivate AP+STA mode by removing the AP connection.
pub async fn deactivate_ap_sta(
    nm: &(impl NetworkManagerConnectionOps + Sync + ?Sized),
    active_connection_path: &str,
) -> Result<(), NetworkError> {
    nm.deactivate_connection(active_connection_path).await?;
    let profile_path = nm.get_connection_profile(active_connection_path).await?;
    nm.delete_connection(&profile_path).await?;
    Ok(())
}

/// Real NetworkManager connection operations via D-Bus.
pub struct RealNetworkManagerConnection {
    connection: Connection,
}

impl RealNetworkManagerConnection {
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
impl NetworkManagerConnectionOps for RealNetworkManagerConnection {
    async fn add_and_activate_connection(
        &self,
        connection_settings: &ConnectionSettings,
        device_path: &str,
    ) -> Result<String, NetworkError> {
        use std::collections::HashMap;
        use zbus::zvariant::{ObjectPath, Value};

        let variant_settings: HashMap<String, HashMap<String, Value<'_>>> = connection_settings
            .iter()
            .map(|(section, inner)| {
                let variant_inner: HashMap<String, Value<'_>> = inner
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::new(v.clone())))
                    .collect();
                (section.clone(), variant_inner)
            })
            .collect();

        let device_path = ObjectPath::try_from(device_path.to_string())
            .map_err(|e| NetworkError::Dbus(format!("invalid device path: {e}")))?;
        let nm_path = ObjectPath::try_from("/").map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            NM_PATH,
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let reply = proxy
            .call_method(
                "AddAndActivateConnection",
                &(variant_settings, device_path, nm_path),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;
        let body = reply.body();
        let (active_path, _): (ObjectPath<'_>, ObjectPath<'_>) = body
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(active_path.to_string())
    }

    async fn deactivate_connection(
        &self,
        active_connection_path: &str,
    ) -> Result<(), NetworkError> {
        let active_path = zbus::zvariant::ObjectPath::try_from(active_connection_path.to_string())
            .map_err(|e| NetworkError::Dbus(format!("invalid active path: {e}")))?;

        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            NM_PATH,
            "org.freedesktop.NetworkManager",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        proxy
            .call_method("DeactivateConnection", &(active_path,))
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(())
    }

    async fn delete_connection(&self, connection_path: &str) -> Result<(), NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            connection_path,
            "org.freedesktop.NetworkManager.Settings.Connection",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        proxy
            .call_method("Delete", &())
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(())
    }

    async fn get_connection_profile(
        &self,
        active_connection_path: &str,
    ) -> Result<String, NetworkError> {
        let proxy = zbus::Proxy::new(
            &self.connection,
            NM_NAME,
            active_connection_path,
            "org.freedesktop.DBus.Properties",
        )
        .await
        .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        let profile: String = proxy
            .call_method(
                "Get",
                &(
                    "org.freedesktop.NetworkManager.Connection.Active",
                    "Connection",
                ),
            )
            .await
            .map_err(|e| NetworkError::Dbus(e.to_string()))?
            .body()
            .deserialize()
            .map_err(|e| NetworkError::Dbus(e.to_string()))?;

        Ok(profile)
    }
}

/// Mock NetworkManager connection operations for testing.
pub struct MockNetworkManagerConnection {
    next_id: std::sync::atomic::AtomicU32,
}

impl MockNetworkManagerConnection {
    pub fn new() -> Self {
        Self {
            next_id: std::sync::atomic::AtomicU32::new(1),
        }
    }
}

#[async_trait::async_trait]
impl NetworkManagerConnectionOps for MockNetworkManagerConnection {
    async fn add_and_activate_connection(
        &self,
        _connection_settings: &ConnectionSettings,
        _device_path: &str,
    ) -> Result<String, NetworkError> {
        let id = self
            .next_id
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!(
            "/org/freedesktop/NetworkManager/ActiveConnection/{id}"
        ))
    }

    async fn deactivate_connection(
        &self,
        _active_connection_path: &str,
    ) -> Result<(), NetworkError> {
        Ok(())
    }

    async fn delete_connection(&self, _connection_path: &str) -> Result<(), NetworkError> {
        Ok(())
    }

    async fn get_connection_profile(
        &self,
        _active_connection_path: &str,
    ) -> Result<String, NetworkError> {
        Ok("/org/freedesktop/NetworkManager/Settings/1".to_string())
    }
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
        self.outputs.insert(key.to_string(), output.to_string());
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
        self.device_types.insert(path.to_string(), DEVICE_TYPE_WIFI);
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

    pub fn add_access_point(&mut self, device_path: &str, ap_path: &str, ssid: &str, strength: u8) {
        self.access_points
            .entry(device_path.to_string())
            .or_default()
            .push(ap_path.to_string());
        self.ap_ssids
            .insert(ap_path.to_string(), ssid.as_bytes().to_vec());
        self.ap_strengths.insert(ap_path.to_string(), strength);
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

    async fn get_active_connection_state(&self, active_path: &str) -> Result<u32, NetworkError> {
        self.connection_states
            .get(active_path)
            .copied()
            .ok_or_else(|| NetworkError::Parse(format!("unknown active connection: {active_path}")))
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
        mock.add_output(
            "iw",
            "Wiphy phy0\nvalid interface combinations:\n * { AP }, #{ managed } <= 1, total <= 2\n",
        );

        let result = check_ap_sta_capability(&mock).await.unwrap();
        assert!(result.supported);
        assert_eq!(result.chipset, Some("phy0".to_string()));
    }

    #[tokio::test]
    async fn check_ap_sta_capability_not_supported() {
        let mut mock = MockCommandRunner::new();
        mock.add_output(
            "iw",
            "Wiphy phy0\nvalid interface combinations:\n * #{ managed } <= 1\n",
        );

        let result = check_ap_sta_capability(&mock).await.unwrap();
        assert!(!result.supported);
    }

    // --- AP+STA configuration tests ---

    #[tokio::test]
    async fn activate_ap_sta_supported() {
        let mock = MockNetworkManagerConnection::new();
        let capability = ApStaCapability {
            supported: true,
            chipset: Some("iwlwifi".to_string()),
            reason: "Found valid AP+STA combination".to_string(),
        };

        let result = activate_ap_sta(
            &mock,
            "TestSSID",
            "password123",
            "5GHz",
            "wlan0",
            &capability,
        )
        .await
        .unwrap();

        assert!(result.success);
        assert!(!result.forced);
        assert!(result.warning.is_none());
    }

    #[tokio::test]
    async fn activate_ap_sta_unsupported_forces_with_warning() {
        let mock = MockNetworkManagerConnection::new();
        let capability = ApStaCapability {
            supported: false,
            chipset: Some("rtl8821ce".to_string()),
            reason: "No valid AP+STA combination found".to_string(),
        };

        let result = activate_ap_sta(
            &mock,
            "TestSSID",
            "password123",
            "5GHz",
            "wlan0",
            &capability,
        )
        .await
        .unwrap();

        assert!(result.success);
        assert!(result.forced);
        assert!(result.warning.is_some());
        assert!(result.warning.unwrap().contains("rtl8821ce"));
    }

    #[tokio::test]
    async fn deactivate_ap_sta_success() {
        let mock = MockNetworkManagerConnection::new();
        let result =
            deactivate_ap_sta(&mock, "/org/freedesktop/NetworkManager/ActiveConnection/1").await;
        assert!(result.is_ok());
    }

    #[test]
    fn build_ap_connection_settings_has_required_fields() {
        let settings = build_ap_connection_settings("MySSID", "mypassword123", "5GHz", "wlan0");

        assert!(settings.contains_key("connection"));
        assert!(settings.contains_key("802-11-wireless"));
        assert!(settings.contains_key("802-11-wireless-security"));
        assert!(settings.contains_key("ipv4"));
        assert!(settings.contains_key("ipv6"));

        let conn = &settings["connection"];
        assert_eq!(conn["id"], "Reecho-MySSID");
        assert_eq!(conn["type"], "802-11-wireless");

        let wireless = &settings["802-11-wireless"];
        assert_eq!(wireless["ssid"], "MySSID");
        assert_eq!(wireless["mode"], "ap");
        assert_eq!(wireless["band"], "5GHz");

        let security = &settings["802-11-wireless-security"];
        assert_eq!(security["key-mgmt"], "wpa-psk");
        assert_eq!(security["psk"], "mypassword123");

        let ipv4 = &settings["ipv4"];
        assert_eq!(ipv4["method"], "shared");
    }
}

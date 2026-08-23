/// D-Bus bus name for the Reecho service.
pub const DBUS_NAME: &str = "org.reecho.Service";

/// D-Bus object path for the Reecho service.
pub const DBUS_PATH: &str = "/org/reecho/Service";

/// D-Bus interface name for the Reecho service.
pub const DBUS_INTERFACE: &str = "org.reecho.Service";

/// Default SSID prefix (appended with hostname at runtime).
pub const DEFAULT_SSID_PREFIX: &str = "Reecho";

/// Minimum password length (WPA2/WPA3 requirement).
pub const MIN_PASSWORD_LEN: usize = 8;

/// Maximum password length (802.11 spec).
pub const MAX_PASSWORD_LEN: usize = 63;

/// Maximum SSID length (802.11 spec).
pub const MAX_SSID_LEN: usize = 32;

/// Default band when not configured.
pub const DEFAULT_BAND: &str = "5GHz";

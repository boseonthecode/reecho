//! D-Bus interface definition and handlers for the Reecho service.

use reecho_shared::HotspotState;

/// The Reecho service D-Bus interface.
pub struct ReechoService {
    state: HotspotState,
    /// Warning message from the last activation (e.g., force mode).
    warning: Option<String>,
}

impl ReechoService {
    /// Create a new service instance in the inactive state.
    pub fn new() -> Self {
        Self {
            state: HotspotState::Inactive,
            warning: None,
        }
    }

    /// Set the current state and optional warning.
    pub fn set_state(&mut self, state: HotspotState, warning: Option<String>) {
        self.state = state;
        self.warning = warning;
    }
}

#[zbus::interface(name = "org.reecho.Service")]
impl ReechoService {
    /// Get the current hotspot state.
    async fn get_state(&self) -> String {
        self.state.to_string()
    }

    /// Get the current warning message, if any.
    async fn get_warning(&self) -> String {
        self.warning.clone().unwrap_or_default()
    }

    /// Activate the hotspot with the given parameters.
    async fn activate(
        &mut self,
        _ssid: String,
        _password: String,
        _band: String,
    ) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 5.
        self.state = HotspotState::Active;
        Ok(())
    }

    /// Deactivate the hotspot.
    async fn deactivate(&mut self) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 5.
        self.state = HotspotState::Inactive;
        self.warning = None;
        Ok(())
    }

    /// Get the list of connected devices.
    async fn get_devices(&self) -> Vec<(String, String, String, String, f64, f64, f64)> {
        // Placeholder — returns empty list.
        Vec::new()
    }

    /// Blacklist a device by MAC address.
    async fn blacklist_device(&mut self, _mac: String) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 6.
        Ok(())
    }

    /// Unblacklist a device by MAC address.
    async fn unblacklist_device(&mut self, _mac: String) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 6.
        Ok(())
    }

    /// Get cumulative data usage (rx, tx, limit).
    async fn get_data_usage(&self) -> (u64, u64, u64) {
        // Placeholder — returns zeros.
        (0, 0, 0)
    }

    /// Set the data usage cap in bytes (0 = unlimited).
    async fn set_data_limit(&mut self, _bytes: u64) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 9.
        Ok(())
    }

    /// Get current hotspot configuration.
    async fn get_config(&self) -> (&str, &str, &str) {
        // Placeholder — returns defaults.
        ("", "", "5GHz")
    }

    /// Set hotspot configuration.
    async fn set_config(
        &mut self,
        _ssid: String,
        _password: String,
        _band: String,
    ) -> Result<(), zbus::fdo::Error> {
        // Placeholder — full implementation in Phase 5.
        Ok(())
    }
}

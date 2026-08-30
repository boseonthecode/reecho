//! D-Bus interface definition and handlers for the Reecho service.

use reecho_shared::{ConnectedDevice, HotspotState, ScheduleEntry};

use crate::blacklist::Blacklist;
use crate::config::Config;
use crate::devices::{self, CommandRunner as _};
use crate::limits::DataLimitTracker;
use crate::scheduler::Scheduler;

/// The Reecho service D-Bus interface.
pub struct ReechoService {
    state: HotspotState,
    /// Warning message from the last activation (e.g., force mode).
    warning: Option<String>,
    /// Current configuration.
    config: Config,
    /// Scheduler for auto on/off.
    scheduler: Scheduler,
    /// Data limit tracker.
    data_tracker: DataLimitTracker,
    /// Device blacklist.
    blacklist: Blacklist,
    /// Connected devices (cached from last poll).
    devices: Vec<ConnectedDevice>,
    /// Name of the active AP interface (set when hotspot is active).
    ap_interface: Option<String>,
    /// Object path of the active NM connection (set when hotspot is active).
    active_connection_path: Option<String>,
}

impl ReechoService {
    /// Create a new service instance in the inactive state.
    pub fn new() -> Self {
        let config = Config::load();
        Self::new_with_config(config)
    }

    /// Create a new service instance with a specific config.
    pub fn new_with_config(config: Config) -> Self {
        let scheduler = Scheduler::from_config(&config);
        let data_tracker = DataLimitTracker::from_config(&config);
        let blacklist = Blacklist::from_macs(config.blacklist.clone());
        Self {
            state: HotspotState::Inactive,
            warning: None,
            config,
            scheduler,
            data_tracker,
            blacklist,
            devices: Vec::new(),
            ap_interface: None,
            active_connection_path: None,
        }
    }

    /// Set the current state and optional warning.
    pub fn set_state(&mut self, state: HotspotState, warning: Option<String>) {
        self.state = state;
        self.warning = warning;
        self.scheduler.mark_state_change();
    }

    /// Get a reference to the scheduler.
    pub fn scheduler(&self) -> &Scheduler {
        &self.scheduler
    }

    /// Get a mutable reference to the scheduler.
    pub fn scheduler_mut(&mut self) -> &mut Scheduler {
        &mut self.scheduler
    }

    /// Get a reference to the data limit tracker.
    pub fn data_tracker(&self) -> &DataLimitTracker {
        &self.data_tracker
    }

    /// Get a mutable reference to the data limit tracker.
    pub fn data_tracker_mut(&mut self) -> &mut DataLimitTracker {
        &mut self.data_tracker
    }

    /// Get the current configuration.
    pub fn config(&self) -> &Config {
        &self.config
    }

    /// Get a mutable reference to the configuration.
    pub fn config_mut(&mut self) -> &mut Config {
        &mut self.config
    }

    /// Get a reference to the blacklist.
    pub fn blacklist(&self) -> &Blacklist {
        &self.blacklist
    }

    /// Get a mutable reference to the blacklist.
    pub fn blacklist_mut(&mut self) -> &mut Blacklist {
        &mut self.blacklist
    }

    /// Persist the blacklist to config and save.
    fn save_blacklist(&mut self) -> Result<(), zbus::fdo::Error> {
        self.config.blacklist = self.blacklist.macs();
        self.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Poll connected devices from hostapd and update internal state.
    pub async fn poll_devices(&mut self) {
        let Some(ref interface) = self.ap_interface else {
            self.devices.clear();
            return;
        };

        let runner = devices::RealCommandRunner;

        // Get connected MACs from hostapd_cli.
        let macs = match runner
            .run_command("hostapd_cli", &["-i", interface, "all_sta"])
            .await
        {
            Ok(output) => devices::parse_all_sta(&output),
            Err(e) => {
                tracing::debug!("failed to poll hostapd_cli all_sta: {e}");
                Vec::new()
            }
        };

        // Get MAC→IP mapping from ip neigh.
        let ip_map = match runner.run_command("ip", &["neigh"]).await {
            Ok(output) => devices::parse_ip_neigh(&output),
            Err(_) => std::collections::HashMap::new(),
        };

        // Build device list.
        let mut new_devices = Vec::new();
        for mac in &macs {
            let ip = ip_map.get(mac).cloned().unwrap_or_default();
            let name = String::new(); // hostname resolution deferred
            let connected_at = String::new(); // tracking deferred

            // Find existing device to carry over bandwidth data.
            let existing = self.devices.iter().find(|d| &d.mac == mac);

            let (bytes_rx, bytes_tx, rate_rx, rate_tx) = match existing {
                Some(dev) => (dev.bytes_rx, dev.bytes_tx, dev.rate_rx, dev.rate_tx),
                None => (0, 0, 0.0, 0.0),
            };

            new_devices.push(ConnectedDevice {
                mac: mac.clone(),
                ip,
                name,
                connected_at,
                bytes_rx,
                bytes_tx,
                rate_rx,
                rate_tx,
            });
        }

        // Enforce blacklist: disconnect blacklisted devices.
        let blacklisted = self.blacklist.filter(&macs);
        for mac in &blacklisted {
            if let Err(e) = runner
                .run_command("hostapd_cli", &["-i", interface, "deauthenticate", mac])
                .await
            {
                tracing::warn!("failed to disconnect blacklisted device {mac}: {e}");
            } else {
                tracing::info!("disconnected blacklisted device: {mac}");
                // Remove from device list.
                new_devices.retain(|d| &d.mac != mac);
            }
        }

        // Emit DeviceDisconnected for devices that left.
        let old_macs: std::collections::HashSet<&str> =
            self.devices.iter().map(|d| d.mac.as_str()).collect();
        let new_macs: std::collections::HashSet<&str> =
            new_devices.iter().map(|d| d.mac.as_str()).collect();

        // Device connected (in new but not in old).
        for mac in &new_macs {
            if !old_macs.contains(mac) {
                tracing::info!("device connected: {mac}");
                // Signal emission deferred — requires zbus SignalContext.
            }
        }

        // Device disconnected (in old but not in new).
        for mac in &old_macs {
            if !new_macs.contains(mac) {
                tracing::info!("device disconnected: {mac}");
                // Signal emission deferred — requires zbus SignalContext.
            }
        }

        self.devices = new_devices;
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
        ssid: String,
        password: String,
        band: String,
    ) -> Result<(), zbus::fdo::Error> {
        // Validate inputs.
        let band: reecho_shared::Band = band
            .parse()
            .map_err(|e: String| zbus::fdo::Error::InvalidArgs(e))?;

        // Check if already active.
        if self.state == HotspotState::Active {
            return Err(zbus::fdo::Error::Failed(
                "hotspot is already active".to_string(),
            ));
        }

        // Update config with provided parameters.
        self.config.ssid = ssid;
        self.config.password = password;
        self.config.band = band;
        self.config
            .validate()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        // Set state to activating.
        self.state = HotspotState::Activating;

        // The actual activation pipeline (NM + hostapd) is orchestrated externally.
        // For now, mark as active. Full pipeline integration requires a running
        // tokio runtime with the ActivationPipeline, which is wired in main.rs.
        self.state = HotspotState::Active;

        // Save config.
        self.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        tracing::info!(
            "hotspot activated: SSID={}, band={}",
            self.config.ssid,
            self.config.band
        );

        Ok(())
    }

    /// Deactivate the hotspot.
    async fn deactivate(&mut self) -> Result<(), zbus::fdo::Error> {
        if self.state == HotspotState::Inactive {
            return Ok(());
        }

        self.state = HotspotState::Inactive;
        self.warning = None;
        self.ap_interface = None;
        self.active_connection_path = None;
        self.devices.clear();

        tracing::info!("hotspot deactivated");
        Ok(())
    }

    /// Get the list of connected devices.
    async fn get_devices(&self) -> Vec<(String, String, String, String, f64, f64, f64)> {
        self.devices
            .iter()
            .map(|d| {
                (
                    d.mac.clone(),
                    d.ip.clone(),
                    d.name.clone(),
                    d.connected_at.clone(),
                    d.bytes_rx as f64,
                    d.bytes_tx as f64,
                    d.rate_rx,
                )
            })
            .collect()
    }

    /// Blacklist a device by MAC address.
    async fn blacklist_device(&mut self, mac: String) -> Result<(), zbus::fdo::Error> {
        if !self.blacklist.add(&mac) {
            return Err(zbus::fdo::Error::Failed(format!(
                "device {mac} is already blacklisted"
            )));
        }
        self.save_blacklist()?;
        tracing::info!("blacklisted device: {mac}");
        Ok(())
    }

    /// Unblacklist a device by MAC address.
    async fn unblacklist_device(&mut self, mac: String) -> Result<(), zbus::fdo::Error> {
        if !self.blacklist.remove(&mac) {
            return Err(zbus::fdo::Error::Failed(format!(
                "device {mac} is not blacklisted"
            )));
        }
        self.save_blacklist()?;
        tracing::info!("unblacklisted device: {mac}");
        Ok(())
    }

    /// Get cumulative data usage (rx, tx, limit).
    async fn get_data_usage(&self) -> (u64, u64, u64) {
        let usage = self.data_tracker.usage();
        (usage.total_rx, usage.total_tx, usage.limit)
    }

    /// Set the data usage cap in bytes (0 = unlimited).
    async fn set_data_limit(&mut self, bytes: u64) -> Result<(), zbus::fdo::Error> {
        self.data_tracker.set_limit(bytes);
        self.config.data_limit = bytes;
        self.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get current hotspot configuration.
    async fn get_config(&self) -> (String, String, String) {
        (
            self.config.ssid.clone(),
            self.config.password.clone(),
            self.config.band.to_string(),
        )
    }

    /// Set hotspot configuration.
    async fn set_config(
        &mut self,
        ssid: String,
        password: String,
        band: String,
    ) -> Result<(), zbus::fdo::Error> {
        self.config.ssid = ssid;
        self.config.password = password;
        self.config.band = band
            .parse()
            .map_err(|e: String| zbus::fdo::Error::InvalidArgs(e))?;
        self.config
            .validate()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        self.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get the current auto on/off schedule.
    async fn get_schedule(&self) -> (String, String, String, bool) {
        match self.scheduler.get_schedule() {
            Some(entry) => (entry.on_time, entry.off_time, entry.repeat, entry.enabled),
            None => (String::new(), String::new(), String::new(), false),
        }
    }

    /// Set the auto on/off schedule.
    async fn set_schedule(
        &mut self,
        on_time: String,
        off_time: String,
        repeat: String,
        enabled: bool,
    ) -> Result<(), zbus::fdo::Error> {
        let entry = ScheduleEntry {
            on_time,
            off_time,
            repeat,
            enabled,
        };
        self.scheduler
            .set_schedule(&entry)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        // Sync back to config.
        self.config.auto_on = if enabled {
            Some(self.scheduler.get_schedule().unwrap().on_time)
        } else {
            None
        };
        self.config.auto_off = if enabled {
            Some(self.scheduler.get_schedule().unwrap().off_time)
        } else {
            None
        };
        self.config.schedule_repeat = self
            .scheduler
            .get_schedule()
            .map(|e| e.repeat)
            .unwrap_or_else(|| "daily".to_string());
        self.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get data usage as a formatted string.
    async fn get_data_usage_string(&self) -> String {
        let usage = self.data_tracker.usage();
        crate::limits::format_bytes(usage.total_rx + usage.total_tx)
    }

    /// Get data limit as a formatted string.
    async fn get_data_limit_string(&self) -> String {
        if self.data_tracker.limit() == 0 {
            "unlimited".to_string()
        } else {
            crate::limits::format_bytes(self.data_tracker.limit())
        }
    }

    /// Check if the data limit is approaching or exceeded.
    async fn check_data_limit(&self) -> String {
        self.data_tracker.check().to_string()
    }

    /// Get the list of blacklisted MAC addresses.
    async fn get_blacklist(&self) -> Vec<String> {
        self.blacklist.macs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Config {
        Config {
            ssid: "Reecho-test".to_string(),
            password: "testpassword123".to_string(),
            band: reecho_shared::Band::Band5Ghz,
            data_limit: 0,
            blacklist: Vec::new(),
            auto_on: None,
            auto_off: None,
            schedule_repeat: "daily".to_string(),
        }
    }

    #[tokio::test]
    async fn get_state_returns_inactive_by_default() {
        let service = ReechoService::new_with_config(test_config());
        assert_eq!(service.get_state().await, "inactive");
    }

    #[tokio::test]
    async fn get_warning_returns_empty_by_default() {
        let service = ReechoService::new_with_config(test_config());
        assert_eq!(service.get_warning().await, "");
    }

    #[tokio::test]
    async fn set_state_updates_state_and_warning() {
        let mut service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, Some("Force mode active".to_string()));
        assert_eq!(service.get_state().await, "active");
        assert_eq!(service.get_warning().await, "Force mode active");
    }

    #[tokio::test]
    async fn deactivate_clears_warning() {
        let mut service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, Some("Force mode active".to_string()));
        service.deactivate().await.unwrap();
        assert_eq!(service.get_state().await, "inactive");
        assert_eq!(service.get_warning().await, "");
    }

    #[tokio::test]
    async fn activate_sets_active_state() {
        let mut service = ReechoService::new_with_config(test_config());
        service
            .activate(
                "Test".to_string(),
                "pass1234".to_string(),
                "5GHz".to_string(),
            )
            .await
            .unwrap();
        assert_eq!(service.get_state().await, "active");
    }

    #[tokio::test]
    async fn activate_rejects_invalid_band() {
        let mut service = ReechoService::new_with_config(test_config());
        let result = service
            .activate(
                "Test".to_string(),
                "pass1234".to_string(),
                "6GHz".to_string(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn activate_rejects_already_active() {
        let mut service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, None);
        let result = service
            .activate(
                "Test".to_string(),
                "pass1234".to_string(),
                "5GHz".to_string(),
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deactivate_from_inactive_is_noop() {
        let mut service = ReechoService::new_with_config(test_config());
        service.deactivate().await.unwrap();
        assert_eq!(service.get_state().await, "inactive");
        assert_eq!(service.get_warning().await, "");
    }

    #[tokio::test]
    async fn deactivate_clears_ap_interface() {
        let mut service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, None);
        service.ap_interface = Some("wlan0".to_string());
        service.active_connection_path = Some("/active/0".to_string());
        service.deactivate().await.unwrap();
        assert!(service.ap_interface.is_none());
        assert!(service.active_connection_path.is_none());
    }

    #[tokio::test]
    async fn blacklist_device_adds_mac() {
        let mut service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        assert!(service.blacklist().is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert!(
            service
                .config
                .blacklist
                .contains(&"aa:bb:cc:dd:ee:ff".to_string())
        );
    }

    #[tokio::test]
    async fn blacklist_device_rejects_duplicate() {
        let mut service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        let result = service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn unblacklist_device_removes_mac() {
        let mut service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        service
            .unblacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        assert!(!service.blacklist().is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert!(service.config.blacklist.is_empty());
    }

    #[tokio::test]
    async fn unblacklist_device_rejects_unknown() {
        let mut service = ReechoService::new_with_config(test_config());
        let result = service
            .unblacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_blacklist_returns_macs() {
        let mut service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        service
            .blacklist_device("11:22:33:44:55:66".to_string())
            .await
            .unwrap();
        let bl = service.get_blacklist().await;
        assert_eq!(bl.len(), 2);
        assert!(bl.contains(&"aa:bb:cc:dd:ee:ff".to_string()));
        assert!(bl.contains(&"11:22:33:44:55:66".to_string()));
    }

    #[tokio::test]
    async fn get_devices_returns_empty_by_default() {
        let service = ReechoService::new_with_config(test_config());
        assert!(service.get_devices().await.is_empty());
    }

    #[tokio::test]
    async fn get_data_usage_returns_zeros_by_default() {
        let service = ReechoService::new_with_config(test_config());
        let (rx, tx, limit) = service.get_data_usage().await;
        assert_eq!(rx, 0);
        assert_eq!(tx, 0);
        assert_eq!(limit, 0);
    }

    #[tokio::test]
    async fn set_data_limit_updates_tracker() {
        let mut service = ReechoService::new_with_config(test_config());
        service.set_data_limit(1_000_000_000).await.unwrap();
        assert_eq!(service.data_tracker().limit(), 1_000_000_000);
    }

    #[tokio::test]
    async fn get_schedule_returns_empty_when_no_schedule() {
        let service = ReechoService::new_with_config(test_config());
        let (on, off, repeat, enabled) = service.get_schedule().await;
        assert!(on.is_empty());
        assert!(!enabled);
    }

    #[tokio::test]
    async fn get_schedule_returns_values_when_set() {
        let mut config = test_config();
        config.auto_on = Some("09:00".to_string());
        config.auto_off = Some("21:00".to_string());
        let service = ReechoService::new_with_config(config);
        let (on, off, repeat, enabled) = service.get_schedule().await;
        assert_eq!(on, "09:00");
        assert_eq!(off, "21:00");
        assert!(enabled);
    }

    #[tokio::test]
    async fn get_data_usage_string_returns_unlimited_when_no_limit() {
        let service = ReechoService::new_with_config(test_config());
        let result = service.get_data_usage_string().await;
        assert_eq!(result, "0 B");
    }

    #[tokio::test]
    async fn get_data_limit_string_returns_unlimited_when_zero() {
        let service = ReechoService::new_with_config(test_config());
        let result = service.get_data_limit_string().await;
        assert_eq!(result, "unlimited");
    }

    #[tokio::test]
    async fn get_data_limit_string_returns_formatted_when_set() {
        let mut config = test_config();
        config.data_limit = 1_073_741_824; // 1 GB
        let service = ReechoService::new_with_config(config);
        let result = service.get_data_limit_string().await;
        assert_eq!(result, "1.00 GB");
    }

    #[tokio::test]
    async fn check_data_limit_returns_ok_when_no_limit() {
        let service = ReechoService::new_with_config(test_config());
        let result = service.check_data_limit().await;
        assert_eq!(result, "ok");
    }
}

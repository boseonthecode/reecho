//! D-Bus interface definition and handlers for the Reecho service.

use reecho_shared::{HotspotState, ScheduleEntry};

use crate::config::Config;
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
}

impl ReechoService {
    /// Create a new service instance in the inactive state.
    pub fn new() -> Self {
        let config = Config::load();
        let scheduler = Scheduler::from_config(&config);
        let data_tracker = DataLimitTracker::from_config(&config);
        Self {
            state: HotspotState::Inactive,
            warning: None,
            config,
            scheduler,
            data_tracker,
        }
    }

    /// Create a new service instance with a specific config (for testing).
    pub fn new_with_config(config: Config) -> Self {
        let scheduler = Scheduler::from_config(&config);
        let data_tracker = DataLimitTracker::from_config(&config);
        Self {
            state: HotspotState::Inactive,
            warning: None,
            config,
            scheduler,
            data_tracker,
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
    async fn deactivate_from_inactive_is_noop() {
        let mut service = ReechoService::new_with_config(test_config());
        service.deactivate().await.unwrap();
        assert_eq!(service.get_state().await, "inactive");
        assert_eq!(service.get_warning().await, "");
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

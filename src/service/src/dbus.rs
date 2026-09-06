//! D-Bus interface definition and handlers for the Reecho service.

use std::sync::Arc;

use tokio::sync::Mutex;
use zbus::object_server::SignalContext;

use reecho_shared::{ConnectedDevice, HotspotState, ScheduleEntry};

use crate::activation::ActivationPipeline;
use crate::ap::HostapdProcess;
use crate::blacklist::Blacklist;
use crate::config::Config;
use crate::devices::{self, CommandRunner as _};
use crate::limits::DataLimitTracker;
use crate::scheduler::Scheduler;

struct Inner {
    state: HotspotState,
    warning: Option<String>,
    config: Config,
    scheduler: Scheduler,
    data_tracker: DataLimitTracker,
    blacklist: Blacklist,
    devices: Vec<ConnectedDevice>,
    ap_interface: Option<String>,
    active_connection_path: Option<String>,
    hostapd_process: Option<HostapdProcess>,
    pipeline: Option<Arc<Mutex<ActivationPipeline>>>,
    connection: Option<zbus::Connection>,
}

impl Inner {
    fn new(config: Config, pipeline: Option<Arc<Mutex<ActivationPipeline>>>) -> Self {
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
            hostapd_process: None,
            pipeline,
            connection: None,
        }
    }
}

/// The Reecho service D-Bus interface.
#[derive(Clone)]
pub struct ReechoService {
    inner: Arc<Mutex<Inner>>,
}

impl ReechoService {
    /// Create a new service instance in the inactive state.
    pub fn new() -> Self {
        let config = Config::load();
        Self::new_with_config(config)
    }

    /// Create a new service instance with a specific config.
    pub fn new_with_config(config: Config) -> Self {
        let inner = Inner::new(config, None);
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Create a new service instance with a pipeline (wired in main.rs).
    pub fn new_with_pipeline(config: Config, pipeline: Arc<Mutex<ActivationPipeline>>) -> Self {
        let inner = Inner::new(config, Some(pipeline));
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Create from an existing Arc<Mutex<Inner>> (for background loops).
    pub fn from_inner(inner: Arc<Mutex<Inner>>) -> Self {
        Self { inner }
    }

    /// Get the shared inner for background tasks.
    pub fn inner(&self) -> Arc<Mutex<Inner>> {
        Arc::clone(&self.inner)
    }

    /// Store the D-Bus connection for signal emission.
    pub async fn set_connection(&self, conn: zbus::Connection) {
        let mut g = self.inner.lock().await;
        g.connection = Some(conn);
    }

    /// Set the current state and optional warning.
    pub async fn set_state(&self, state: HotspotState, warning: Option<String>) {
        let conn = {
            let mut g = self.inner.lock().await;
            g.state = state;
            g.warning = warning;
            g.scheduler.mark_state_change();
            g.connection.clone()
        };
        let state_str = self.get_state().await;
        if let Some(conn) = conn {
            let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                .unwrap()
                .into();
            let ctxt = SignalContext::from_parts(conn, path);
            let _ = Self::state_changed(&ctxt, &state_str).await;
        }
    }

    /// Get a reference to the scheduler (cloned).
    pub async fn scheduler_snapshot(&self) -> Scheduler {
        let g = self.inner.lock().await;
        g.scheduler.clone()
    }

    /// Helper to emit signals
    async fn emit_state_changed(conn: &Option<zbus::Connection>, state: &str) {
        if let Some(c) = conn {
            let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                .unwrap()
                .into();
            let ctxt = SignalContext::from_parts(c.clone(), path);
            let _ = Self::state_changed(&ctxt, state).await;
        }
    }

    async fn emit_warning(conn: &Option<zbus::Connection>, msg: &str) {
        if let Some(c) = conn {
            let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                .unwrap()
                .into();
            let ctxt = SignalContext::from_parts(c.clone(), path);
            let _ = Self::warning(&ctxt, msg).await;
        }
    }

    /// Poll connected devices from hostapd and update internal state.
    pub async fn poll_devices(&self) {
        let (iface_opt, conn) = {
            let g = self.inner.lock().await;
            (g.ap_interface.clone(), g.connection.clone())
        };
        let Some(interface) = iface_opt else {
            let mut g = self.inner.lock().await;
            g.devices.clear();
            return;
        };

        let runner = devices::RealCommandRunner;

        let macs = match runner
            .run_command("hostapd_cli", &["-i", &interface, "all_sta"])
            .await
        {
            Ok(output) => devices::parse_all_sta(&output),
            Err(e) => {
                tracing::debug!("failed to poll hostapd_cli all_sta: {e}");
                Vec::new()
            }
        };

        let ip_map = match runner.run_command("ip", &["neigh"]).await {
            Ok(output) => devices::parse_ip_neigh(&output),
            Err(_) => std::collections::HashMap::new(),
        };

        let (old_macs, blacklisted) = {
            let g = self.inner.lock().await;
            let old: std::collections::HashSet<String> =
                g.devices.iter().map(|d| d.mac.clone()).collect();
            let bl = g.blacklist.filter(&macs);
            (old, bl)
        };

        for mac in &blacklisted {
            if let Err(e) = runner
                .run_command("hostapd_cli", &["-i", &interface, "deauthenticate", mac])
                .await
            {
                tracing::warn!("failed to disconnect blacklisted device {mac}: {e}");
            } else {
                tracing::info!("disconnected blacklisted device: {mac}");
            }
        }

        let filtered_macs: Vec<String> = macs
            .into_iter()
            .filter(|m| !blacklisted.contains(m))
            .collect();

        let mut new_devices = Vec::new();
        {
            let g = self.inner.lock().await;
            for mac in &filtered_macs {
                let ip = ip_map.get(mac).cloned().unwrap_or_default();
                let existing = g.devices.iter().find(|d| &d.mac == mac);
                let (bytes_rx, bytes_tx, rate_rx, rate_tx) = match existing {
                    Some(dev) => (dev.bytes_rx, dev.bytes_tx, dev.rate_rx, dev.rate_tx),
                    None => (0, 0, 0.0, 0.0),
                };
                new_devices.push(ConnectedDevice {
                    mac: mac.clone(),
                    ip,
                    name: String::new(),
                    connected_at: String::new(),
                    bytes_rx,
                    bytes_tx,
                    rate_rx,
                    rate_tx,
                });
            }
        }

        let new_macs_set: std::collections::HashSet<String> =
            new_devices.iter().map(|d| d.mac.clone()).collect();

        {
            let mut g = self.inner.lock().await;
            g.devices = new_devices;
        }

        for mac in &new_macs_set {
            if !old_macs.contains(mac) {
                tracing::info!("device connected: {mac}");
                if let Some(c) = &conn {
                    let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                        .unwrap()
                        .into();
                    let ctxt = SignalContext::from_parts(c.clone(), path);
                    let _ = Self::device_connected(&ctxt, mac, "").await;
                }
            }
        }

        for mac in &old_macs {
            if !new_macs_set.contains(mac) {
                tracing::info!("device disconnected: {mac}");
                if let Some(c) = &conn {
                    let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                        .unwrap()
                        .into();
                    let ctxt = SignalContext::from_parts(c.clone(), path);
                    let _ = Self::device_disconnected(&ctxt, mac).await;
                }
            }
        }
    }

    /// Persist the blacklist to config and save.
    async fn save_blacklist_locked(g: &mut Inner) -> Result<(), zbus::fdo::Error> {
        g.config.blacklist = g.blacklist.macs();
        g.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))
    }

    /// Whether hotspot is active.
    pub async fn is_active(&self) -> bool {
        let g = self.inner.lock().await;
        g.state == HotspotState::Active
    }

    /// Compute duration until next scheduled event, if any.
    pub async fn next_schedule_delay(&self, is_active: bool) -> Option<std::time::Duration> {
        let g = self.inner.lock().await;
        let next = g.scheduler.next_event_time(is_active)?;
        let now = crate::scheduler::now_epoch_secs();
        if next <= now {
            return Some(std::time::Duration::from_secs(0));
        }
        Some(std::time::Duration::from_secs(next - now))
    }

    /// Handle a scheduler tick: activate or deactivate if due.
    pub async fn handle_schedule_tick(&self) {
        let is_active = self.is_active().await;
        let (on_time, off_time, enabled) = {
            let g = self.inner.lock().await;
            match g.scheduler.get_schedule() {
                Some(e) => (e.on_time, e.off_time, e.enabled),
                None => return,
            }
        };
        if !enabled {
            return;
        }
        if is_active {
            tracing::info!("scheduler: auto-off triggered ({off_time})");
            let _ = self.deactivate().await;
        } else {
            tracing::info!("scheduler: auto-on triggered ({on_time})");
            let (ssid, password, band) = {
                let g = self.inner.lock().await;
                (
                    g.config.ssid.clone(),
                    g.config.password.clone(),
                    g.config.band.to_string(),
                )
            };
            let _ = self.activate(ssid, password, band).await;
        }
    }

    /// Poll devices and enforce data limit (auto-deactivate + signal).
    pub async fn poll_and_enforce(&self) {
        if !self.is_active().await {
            return;
        }
        self.poll_devices().await;

        {
            let iface = {
                let g = self.inner.lock().await;
                g.ap_interface.clone()
            };
            if let Some(iface) = iface {
                if let Ok(content) = tokio::fs::read_to_string("/proc/net/dev").await {
                    if let Some(c) = devices::parse_proc_net_dev(&content, &iface) {
                        let mut g = self.inner.lock().await;
                        g.data_tracker.update(c.rx_bytes, c.tx_bytes);
                    }
                }
            }
        }

        let (action, conn) = {
            let g = self.inner.lock().await;
            (g.data_tracker.check(), g.connection.clone())
        };

        match action {
            crate::limits::LimitAction::Exceeded => {
                tracing::warn!("data limit exceeded — auto-deactivating");
                if let Some(c) = &conn {
                    let path = zbus::zvariant::ObjectPath::try_from(reecho_shared::DBUS_PATH)
                        .unwrap()
                        .into();
                    let ctxt = SignalContext::from_parts(c.clone(), path);
                    let _ = Self::data_limit_reached(&ctxt).await;
                }
                let _ = self.deactivate().await;
            }
            crate::limits::LimitAction::Approaching { percentage } => {
                tracing::warn!("data limit approaching: {percentage:.1}%");
                Self::emit_warning(&conn, &format!("data limit {percentage:.1}% used")).await;
            }
            crate::limits::LimitAction::None => {}
        }
    }
}

#[zbus::interface(name = "org.reecho.Service")]
impl ReechoService {
    /// Get the current hotspot state.
    async fn get_state(&self) -> String {
        let g = self.inner.lock().await;
        g.state.to_string()
    }

    /// Get the current warning message, if any.
    async fn get_warning(&self) -> String {
        let g = self.inner.lock().await;
        g.warning.clone().unwrap_or_default()
    }

    /// Activate the hotspot with the given parameters.
    async fn activate(
        &self,
        ssid: String,
        password: String,
        band: String,
    ) -> Result<(), zbus::fdo::Error> {
        let band_parsed: reecho_shared::Band = band
            .parse()
            .map_err(|e: String| zbus::fdo::Error::InvalidArgs(e))?;

        {
            let g = self.inner.lock().await;
            if g.state == HotspotState::Active {
                return Err(zbus::fdo::Error::Failed(
                    "hotspot is already active".to_string(),
                ));
            }
        }

        let mut cfg = {
            let g = self.inner.lock().await;
            g.config.clone()
        };
        cfg.ssid = ssid;
        cfg.password = password;
        cfg.band = band_parsed;
        cfg.validate()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;

        {
            let mut g = self.inner.lock().await;
            g.state = HotspotState::Activating;
            g.config = cfg.clone();
        }
        Self::emit_state_changed(&self.inner.lock().await.connection.clone(), "activating").await;

        let pipeline_opt = {
            let g = self.inner.lock().await;
            g.pipeline.clone()
        };

        if let Some(pipeline) = pipeline_opt {
            let pipeline = Arc::clone(&pipeline);
            let result = {
                let p = pipeline.lock().await;
                p.activate(&cfg).await
            };

            match result {
                Ok(res) => {
                    let warning = res.warning.clone();
                    let ap_iface = res.ap_interface.clone();
                    let conn_clone = {
                        let mut g = self.inner.lock().await;
                        g.state = HotspotState::Active;
                        g.warning = warning.clone();
                        g.ap_interface = ap_iface.clone();
                        g.hostapd_process = res.process;
                        g.active_connection_path = None;
                        g.scheduler.mark_state_change();
                        if let Err(e) = g.config.save() {
                            tracing::warn!("failed to save config: {e}");
                        }
                        g.connection.clone()
                    };
                    Self::emit_state_changed(&conn_clone, "active").await;
                    if let Some(w) = warning {
                        Self::emit_warning(&conn_clone, &w).await;
                    }
                    tracing::info!("hotspot activated: SSID={}, band={}", cfg.ssid, cfg.band);
                    Ok(())
                }
                Err(e) => {
                    let conn = {
                        let mut g = self.inner.lock().await;
                        g.state = HotspotState::Failed;
                        g.warning = Some(e.to_string());
                        g.connection.clone()
                    };
                    Self::emit_state_changed(&conn, "failed").await;
                    Err(zbus::fdo::Error::Failed(e.to_string()))
                }
            }
        } else {
            let mut g = self.inner.lock().await;
            g.state = HotspotState::Active;
            g.scheduler.mark_state_change();
            let conn = g.connection.clone();
            if let Err(e) = g.config.save() {
                tracing::warn!("failed to save config: {e}");
            }
            drop(g);
            Self::emit_state_changed(&conn, "active").await;
            tracing::info!(
                "hotspot activated (stub): SSID={}, band={}",
                cfg.ssid,
                cfg.band
            );
            Ok(())
        }
    }

    /// Deactivate the hotspot.
    async fn deactivate(&self) -> Result<(), zbus::fdo::Error> {
        let (was_inactive, process, active_path, pipeline_opt, conn) = {
            let g = self.inner.lock().await;
            (
                g.state == HotspotState::Inactive,
                g.hostapd_process.is_none(),
                g.active_connection_path.clone(),
                g.pipeline.clone(),
                g.connection.clone(),
            )
        };

        if was_inactive {
            return Ok(());
        }

        if let (Some(pipeline), false) = (pipeline_opt, process) {
            let (proc, path) = {
                let g = self.inner.lock().await;
                (
                    g.hostapd_process.as_ref().map(|p| HostapdProcess {
                        config_path: p.config_path.clone(),
                        pid: p.pid,
                    }),
                    g.active_connection_path.clone().unwrap_or_default(),
                )
            };
            if let Some(proc) = proc {
                let p = pipeline.lock().await;
                if let Err(e) = p.deactivate(&proc, &path).await {
                    tracing::warn!("pipeline deactivate failed: {e}");
                }
            } else if !active_path.clone().unwrap_or_default().is_empty() {
                let p = pipeline.lock().await;
                let dummy = HostapdProcess {
                    config_path: std::path::PathBuf::from("/tmp/reecho-dummy.conf"),
                    pid: 0,
                };
                let _ = p
                    .deactivate(&dummy, &active_path.clone().unwrap_or_default())
                    .await;
            }
        }

        {
            let mut g = self.inner.lock().await;
            g.state = HotspotState::Inactive;
            g.warning = None;
            g.ap_interface = None;
            g.active_connection_path = None;
            g.hostapd_process = None;
            g.devices.clear();
        }
        Self::emit_state_changed(&conn, "inactive").await;
        tracing::info!("hotspot deactivated");
        Ok(())
    }

    /// Get the list of connected devices.
    async fn get_devices(&self) -> Vec<(String, String, String, String, f64, f64, f64)> {
        let g = self.inner.lock().await;
        g.devices
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
    async fn blacklist_device(&self, mac: String) -> Result<(), zbus::fdo::Error> {
        let mut g = self.inner.lock().await;
        if !g.blacklist.add(&mac) {
            return Err(zbus::fdo::Error::Failed(format!(
                "device {mac} is already blacklisted"
            )));
        }
        Self::save_blacklist_locked(&mut g).await?;
        tracing::info!("blacklisted device: {mac}");
        Ok(())
    }

    /// Unblacklist a device by MAC address.
    async fn unblacklist_device(&self, mac: String) -> Result<(), zbus::fdo::Error> {
        let mut g = self.inner.lock().await;
        if !g.blacklist.remove(&mac) {
            return Err(zbus::fdo::Error::Failed(format!(
                "device {mac} is not blacklisted"
            )));
        }
        Self::save_blacklist_locked(&mut g).await?;
        tracing::info!("unblacklisted device: {mac}");
        Ok(())
    }

    /// Get cumulative data usage (rx, tx, limit).
    async fn get_data_usage(&self) -> (u64, u64, u64) {
        let g = self.inner.lock().await;
        let usage = g.data_tracker.usage();
        (usage.total_rx, usage.total_tx, usage.limit)
    }

    /// Set the data usage cap in bytes (0 = unlimited).
    async fn set_data_limit(&self, bytes: u64) -> Result<(), zbus::fdo::Error> {
        let mut g = self.inner.lock().await;
        g.data_tracker.set_limit(bytes);
        g.config.data_limit = bytes;
        g.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get current hotspot configuration.
    async fn get_config(&self) -> (String, String, String) {
        let g = self.inner.lock().await;
        (
            g.config.ssid.clone(),
            g.config.password.clone(),
            g.config.band.to_string(),
        )
    }

    /// Set hotspot configuration.
    async fn set_config(
        &self,
        ssid: String,
        password: String,
        band: String,
    ) -> Result<(), zbus::fdo::Error> {
        let mut g = self.inner.lock().await;
        g.config.ssid = ssid;
        g.config.password = password;
        g.config.band = band
            .parse()
            .map_err(|e: String| zbus::fdo::Error::InvalidArgs(e))?;
        g.config
            .validate()
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        g.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get the current auto on/off schedule.
    async fn get_schedule(&self) -> (String, String, String, bool) {
        let g = self.inner.lock().await;
        match g.scheduler.get_schedule() {
            Some(entry) => (entry.on_time, entry.off_time, entry.repeat, entry.enabled),
            None => (String::new(), String::new(), String::new(), false),
        }
    }

    /// Set the auto on/off schedule.
    async fn set_schedule(
        &self,
        on_time: String,
        off_time: String,
        repeat: String,
        enabled: bool,
    ) -> Result<(), zbus::fdo::Error> {
        let mut g = self.inner.lock().await;
        let entry = ScheduleEntry {
            on_time,
            off_time,
            repeat,
            enabled,
        };
        g.scheduler
            .set_schedule(&entry)
            .map_err(|e| zbus::fdo::Error::InvalidArgs(e.to_string()))?;
        g.config.auto_on = if enabled {
            Some(g.scheduler.get_schedule().unwrap().on_time)
        } else {
            None
        };
        g.config.auto_off = if enabled {
            Some(g.scheduler.get_schedule().unwrap().off_time)
        } else {
            None
        };
        g.config.schedule_repeat = g
            .scheduler
            .get_schedule()
            .map(|e| e.repeat)
            .unwrap_or_else(|| "daily".to_string());
        g.config
            .save()
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;
        Ok(())
    }

    /// Get data usage as a formatted string.
    async fn get_data_usage_string(&self) -> String {
        let g = self.inner.lock().await;
        let usage = g.data_tracker.usage();
        crate::limits::format_bytes(usage.total_rx + usage.total_tx)
    }

    /// Get data limit as a formatted string.
    async fn get_data_limit_string(&self) -> String {
        let g = self.inner.lock().await;
        if g.data_tracker.limit() == 0 {
            "unlimited".to_string()
        } else {
            crate::limits::format_bytes(g.data_tracker.limit())
        }
    }

    /// Check if the data limit is approaching or exceeded.
    async fn check_data_limit(&self) -> String {
        let g = self.inner.lock().await;
        g.data_tracker.check().to_string()
    }

    /// Get the list of blacklisted MAC addresses.
    async fn get_blacklist(&self) -> Vec<String> {
        let g = self.inner.lock().await;
        g.blacklist.macs()
    }

    #[zbus(signal)]
    async fn state_changed(ctxt: &SignalContext<'_>, state: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn device_connected(ctxt: &SignalContext<'_>, mac: &str, name: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn device_disconnected(ctxt: &SignalContext<'_>, mac: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn warning(ctxt: &SignalContext<'_>, message: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn data_limit_reached(ctxt: &SignalContext<'_>) -> zbus::Result<()>;
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
        let service = ReechoService::new_with_config(test_config());
        service
            .set_state(HotspotState::Active, Some("Force mode active".to_string()))
            .await;
        assert_eq!(service.get_state().await, "active");
        assert_eq!(service.get_warning().await, "Force mode active");
    }

    #[tokio::test]
    async fn deactivate_clears_warning() {
        let service = ReechoService::new_with_config(test_config());
        service
            .set_state(HotspotState::Active, Some("Force mode active".to_string()))
            .await;
        service.deactivate().await.unwrap();
        assert_eq!(service.get_state().await, "inactive");
        assert_eq!(service.get_warning().await, "");
    }

    #[tokio::test]
    async fn activate_sets_active_state() {
        let service = ReechoService::new_with_config(test_config());
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
        let service = ReechoService::new_with_config(test_config());
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
        let service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, None).await;
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
        let service = ReechoService::new_with_config(test_config());
        service.deactivate().await.unwrap();
        assert_eq!(service.get_state().await, "inactive");
        assert_eq!(service.get_warning().await, "");
    }

    #[tokio::test]
    async fn deactivate_clears_ap_interface() {
        let service = ReechoService::new_with_config(test_config());
        service.set_state(HotspotState::Active, None).await;
        {
            let mut g = service.inner.lock().await;
            g.ap_interface = Some("wlan0".to_string());
            g.active_connection_path = Some("/active/0".to_string());
        }
        service.deactivate().await.unwrap();
        let g = service.inner.lock().await;
        assert!(g.ap_interface.is_none());
        assert!(g.active_connection_path.is_none());
    }

    #[tokio::test]
    async fn blacklist_device_adds_mac() {
        let service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        let g = service.inner.lock().await;
        assert!(g.blacklist.is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert!(
            g.config
                .blacklist
                .contains(&"aa:bb:cc:dd:ee:ff".to_string())
        );
    }

    #[tokio::test]
    async fn blacklist_device_rejects_duplicate() {
        let service = ReechoService::new_with_config(test_config());
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
        let service = ReechoService::new_with_config(test_config());
        service
            .blacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        service
            .unblacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await
            .unwrap();
        let g = service.inner.lock().await;
        assert!(!g.blacklist.is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert!(g.config.blacklist.is_empty());
    }

    #[tokio::test]
    async fn unblacklist_device_rejects_unknown() {
        let service = ReechoService::new_with_config(test_config());
        let result = service
            .unblacklist_device("AA:BB:CC:DD:EE:FF".to_string())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn get_blacklist_returns_macs() {
        let service = ReechoService::new_with_config(test_config());
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
        let service = ReechoService::new_with_config(test_config());
        service.set_data_limit(1_000_000_000).await.unwrap();
        let g = service.inner.lock().await;
        assert_eq!(g.data_tracker.limit(), 1_000_000_000);
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

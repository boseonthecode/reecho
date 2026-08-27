//! Connected device tracking and bandwidth monitoring.
//!
//! Polls `hostapd_cli all_sta` for connected devices, resolves IPs via `ip neigh`,
//! tracks per-device bandwidth via `/proc/net/dev`, and maintains cumulative data usage.

use std::collections::HashMap;

use reecho_shared::ConnectedDevice;

use crate::error::ServiceError;

/// Trait for running shell commands, allowing mocking.
#[async_trait::async_trait]
pub trait CommandRunner {
    async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, ServiceError>;
}

/// Real command runner using tokio::process.
pub struct RealCommandRunner;

#[async_trait::async_trait]
impl CommandRunner for RealCommandRunner {
    async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, ServiceError> {
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| ServiceError::Network(format!("{command} failed: {e}")))?;

        String::from_utf8(output.stdout)
            .map_err(|e| ServiceError::Network(format!("invalid UTF-8 from {command}: {e}")))
    }
}

/// Parse `hostapd_cli all_sta` output into a list of MAC addresses.
///
/// Example output:
/// ```text
/// 00:11:22:33:44:55
/// aa:bb:cc:dd:ee:ff
/// ```
pub fn parse_all_sta(output: &str) -> Vec<String> {
    output
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .filter(|l| l.contains(':') && l.len() == 17)
        .map(|l| l.to_lowercase())
        .collect()
}

/// Parse `ip neigh` output to build a MAC → IP mapping.
///
/// Example output:
/// ```text
/// 192.168.4.2 dev wlan0 lladdr 00:11:22:33:44:55 REACHABLE
/// 192.168.4.3 dev wlan0 lladdr aa:bb:cc:dd:ee:ff STALE
/// ```
pub fn parse_ip_neigh(output: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }
        let ip = parts[0];
        let lladdr_idx = parts.iter().position(|&p| p == "lladdr");
        if let Some(idx) = lladdr_idx {
            if idx + 1 < parts.len() {
                let mac = parts[idx + 1].to_lowercase();
                map.insert(mac, ip.to_string());
            }
        }
    }
    map
}

/// Resolve a hostname from a MAC address using reverse DNS or `/proc/net/arp`.
///
/// Falls back to the MAC address if resolution fails.
pub async fn resolve_hostname(mac: &str, _runner: &(impl CommandRunner + Sync)) -> String {
    // Try to get hostname from `avahi-resolve` or system ARP table.
    // For v1, just return the MAC as the name.
    mac.to_string()
}

/// Get connected devices from hostapd.
pub async fn get_connected_devices(
    interface: &str,
    runner: &(impl CommandRunner + Sync),
) -> Result<Vec<ConnectedDevice>, ServiceError> {
    // 1. Get associated stations.
    let sta_output = runner
        .run_command("hostapd_cli", &["-i", interface, "all_sta"])
        .await?;
    let macs = parse_all_sta(&sta_output);

    if macs.is_empty() {
        return Ok(Vec::new());
    }

    // 2. Get IP mappings from `ip neigh`.
    let neigh_output = runner.run_command("ip", &["neigh"]).await?;
    let mac_to_ip = parse_ip_neigh(&neigh_output);

    // 3. Build device list.
    let mut devices = Vec::new();
    for mac in &macs {
        let ip = mac_to_ip.get(mac).cloned().unwrap_or_default();
        let name = resolve_hostname(mac, runner).await;

        devices.push(ConnectedDevice {
            mac: mac.clone(),
            ip,
            name,
            connected_at: String::new(),
            bytes_rx: 0,
            bytes_tx: 0,
            rate_rx: 0.0,
            rate_tx: 0.0,
        });
    }

    Ok(devices)
}

// --- Bandwidth Monitoring ---

/// Network interface byte counters from `/proc/net/dev`.
#[derive(Debug, Clone, Default)]
pub struct InterfaceCounters {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

/// Parse `/proc/net/dev` to extract byte counters for a specific interface.
///
/// Example line:
/// ```text
/// wlan0: 1234567 890 0 0 0 0 0 0 2345678 901 0 0 0 0 0 0
/// ```
pub fn parse_proc_net_dev(output: &str, interface: &str) -> Option<InterfaceCounters> {
    for line in output.lines() {
        let line = line.trim();
        // Lines starting with the interface name followed by colon.
        if let Some(rest) = line.strip_prefix(&format!("{interface}:")) {
            let fields: Vec<&str> = rest.split_whitespace().collect();
            if fields.len() >= 10 {
                let rx_bytes = fields[0].parse::<u64>().unwrap_or(0);
                let tx_bytes = fields[8].parse::<u64>().unwrap_or(0);
                return Some(InterfaceCounters { rx_bytes, tx_bytes });
            }
        }
    }
    None
}

/// Per-device bandwidth tracking with rolling average.
#[derive(Debug, Clone)]
pub struct DeviceBandwidth {
    /// Total bytes received.
    pub bytes_rx: u64,
    /// Total bytes transmitted.
    pub bytes_tx: u64,
    /// Rolling average receive rate (bytes/sec).
    pub rate_rx: f64,
    /// Rolling average transmit rate (bytes/sec).
    pub rate_tx: f64,
    /// Previous sample for rate calculation.
    prev_rx: u64,
    prev_tx: u64,
    /// Number of samples taken.
    samples: u32,
}

impl DeviceBandwidth {
    pub fn new() -> Self {
        Self {
            bytes_rx: 0,
            bytes_tx: 0,
            rate_rx: 0.0,
            rate_tx: 0.0,
            prev_rx: 0,
            prev_tx: 0,
            samples: 0,
        }
    }

    /// Update with new byte counters and calculate rate.
    ///
    /// `delta_secs` is the time since the last update.
    pub fn update(&mut self, rx: u64, tx: u64, delta_secs: f64) {
        self.bytes_rx = rx;
        self.bytes_tx = tx;

        if self.samples > 0 && delta_secs > 0.0 {
            let drx = rx.saturating_sub(self.prev_rx) as f64;
            let dtx = tx.saturating_sub(self.prev_tx) as f64;

            // Exponential moving average (alpha = 0.3).
            let alpha = 0.3;
            if self.samples == 1 {
                self.rate_rx = drx / delta_secs;
                self.rate_tx = dtx / delta_secs;
            } else {
                let instant_rx = drx / delta_secs;
                let instant_tx = dtx / delta_secs;
                self.rate_rx = alpha * instant_rx + (1.0 - alpha) * self.rate_rx;
                self.rate_tx = alpha * instant_tx + (1.0 - alpha) * self.rate_tx;
            }
        }

        self.prev_rx = rx;
        self.prev_tx = tx;
        self.samples += 1;
    }
}

/// Trait for reading network interface counters, allowing mocking.
pub trait NetDevReader {
    fn read_counters(&self, interface: &str) -> Result<InterfaceCounters, ServiceError>;
}

/// Real reader from `/proc/net/dev`.
pub struct ProcNetDevReader;

impl NetDevReader for ProcNetDevReader {
    fn read_counters(&self, interface: &str) -> Result<InterfaceCounters, ServiceError> {
        let content = std::fs::read_to_string("/proc/net/dev")
            .map_err(|e| ServiceError::Network(format!("failed to read /proc/net/dev: {e}")))?;
        parse_proc_net_dev(&content, interface)
            .ok_or_else(|| ServiceError::Network(format!("interface {interface} not found")))
    }
}

/// Mock reader for testing.
pub struct MockNetDevReader {
    counters: HashMap<String, InterfaceCounters>,
}

impl MockNetDevReader {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
        }
    }

    pub fn set_counters(&mut self, interface: &str, rx: u64, tx: u64) {
        self.counters.insert(
            interface.to_string(),
            InterfaceCounters {
                rx_bytes: rx,
                tx_bytes: tx,
            },
        );
    }
}

impl NetDevReader for MockNetDevReader {
    fn read_counters(&self, interface: &str) -> Result<InterfaceCounters, ServiceError> {
        self.counters
            .get(interface)
            .cloned()
            .ok_or_else(|| ServiceError::Network(format!("interface {interface} not found")))
    }
}

// --- Data Usage Tracking ---

/// Cumulative data usage across all devices.
#[derive(Debug, Clone, Default)]
pub struct DataUsageTracker {
    pub total_rx: u64,
    pub total_tx: u64,
    pub limit: u64,
}

impl DataUsageTracker {
    pub fn new(limit: u64) -> Self {
        Self {
            total_rx: 0,
            total_tx: 0,
            limit,
        }
    }

    /// Update cumulative totals from interface counters.
    pub fn update(&mut self, rx: u64, tx: u64) {
        self.total_rx = rx;
        self.total_tx = tx;
    }

    /// Check if the data limit has been reached.
    pub fn limit_reached(&self) -> bool {
        self.limit > 0 && (self.total_rx + self.total_tx) >= self.limit
    }

    /// Remaining bytes before limit (u64::MAX if no limit).
    pub fn remaining(&self) -> u64 {
        if self.limit == 0 {
            u64::MAX
        } else {
            self.limit.saturating_sub(self.total_rx + self.total_tx)
        }
    }
}

/// Device tracker that polls hostapd and maintains device state.
pub struct DeviceTracker {
    interface: String,
    devices: HashMap<String, ConnectedDevice>,
    bandwidth: HashMap<String, DeviceBandwidth>,
    pub data_usage: DataUsageTracker,
}

impl DeviceTracker {
    pub fn new(interface: String, data_limit: u64) -> Self {
        Self {
            interface,
            devices: HashMap::new(),
            bandwidth: HashMap::new(),
            data_usage: DataUsageTracker::new(data_limit),
        }
    }

    /// Poll connected devices and update state.
    pub async fn poll(
        &mut self,
        runner: &(impl CommandRunner + Sync),
    ) -> Result<Vec<ConnectedDevice>, ServiceError> {
        let devices = get_connected_devices(&self.interface, runner).await?;

        // Update device list.
        let mut current_macs = std::collections::HashSet::new();
        for device in &devices {
            current_macs.insert(device.mac.clone());
            self.devices
                .entry(device.mac.clone())
                .or_insert_with(|| device.clone());
        }

        // Remove devices that are no longer connected.
        self.devices.retain(|mac, _| current_macs.contains(mac));
        self.bandwidth.retain(|mac, _| current_macs.contains(mac));

        // Return current device list.
        Ok(self
            .devices
            .values()
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .collect())
    }

    /// Get the current device list.
    pub fn devices(&self) -> Vec<ConnectedDevice> {
        self.devices.values().cloned().collect()
    }

    /// Update bandwidth for a specific device.
    pub fn update_bandwidth(&mut self, mac: &str, rx: u64, tx: u64, delta_secs: f64) {
        let entry = self
            .bandwidth
            .entry(mac.to_string())
            .or_insert_with(DeviceBandwidth::new);
        entry.update(rx, tx, delta_secs);

        // Sync to device.
        if let Some(device) = self.devices.get_mut(mac) {
            device.bytes_rx = entry.bytes_rx;
            device.bytes_tx = entry.bytes_tx;
            device.rate_rx = entry.rate_rx;
            device.rate_tx = entry.rate_tx;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // --- Parse tests ---

    #[test]
    fn parse_all_sta_with_devices() {
        let output = "00:11:22:33:44:55\naa:bb:cc:dd:ee:ff\n";
        let macs = parse_all_sta(output);
        assert_eq!(macs.len(), 2);
        assert!(macs.contains(&"00:11:22:33:44:55".to_string()));
        assert!(macs.contains(&"aa:bb:cc:dd:ee:ff".to_string()));
    }

    #[test]
    fn parse_all_sta_empty() {
        let macs = parse_all_sta("");
        assert!(macs.is_empty());
    }

    #[test]
    fn parse_all_sta_skips_invalid() {
        let output = "00:11:22:33:44:55\ninvalid\naa:bb:cc:dd:ee:ff\n";
        let macs = parse_all_sta(output);
        assert_eq!(macs.len(), 2);
    }

    #[test]
    fn parse_ip_neigh_with_entries() {
        let output = "192.168.4.2 dev wlan0 lladdr 00:11:22:33:44:55 REACHABLE\n\
                       192.168.4.3 dev wlan0 lladdr aa:bb:cc:dd:ee:ff STALE\n";
        let map = parse_ip_neigh(output);
        assert_eq!(map.len(), 2);
        assert_eq!(
            map.get("00:11:22:33:44:55").map(|s| s.as_str()),
            Some("192.168.4.2")
        );
        assert_eq!(
            map.get("aa:bb:cc:dd:ee:ff").map(|s| s.as_str()),
            Some("192.168.4.3")
        );
    }

    #[test]
    fn parse_ip_neigh_empty() {
        let map = parse_ip_neigh("");
        assert!(map.is_empty());
    }

    #[test]
    fn parse_ip_neigh_skips_incomplete() {
        let output = "192.168.4.2 dev wlan0 REACHABLE\n";
        let map = parse_ip_neigh(output);
        assert!(map.is_empty());
    }

    // --- /proc/net/dev parsing ---

    #[test]
    fn parse_proc_net_dev_extracts_counters() {
        let output = "Inter-|   Receive                                                |  Transmit\n\
                       face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets errs drop fifo frame compressed\n\
                       wlan0: 12345678 90123 0 0 0 0 0 0 23456789 10234 0 0 0 0 0 0\n";
        let counters = parse_proc_net_dev(output, "wlan0").unwrap();
        assert_eq!(counters.rx_bytes, 12345678);
        assert_eq!(counters.tx_bytes, 23456789);
    }

    #[test]
    fn parse_proc_net_dev_interface_not_found() {
        let output = "wlan0: 1234 5678 0 0 0 0 0 0 2345 6789 0 0 0 0 0 0\n";
        let result = parse_proc_net_dev(output, "eth0");
        assert!(result.is_none());
    }

    // --- Bandwidth calculation ---

    #[test]
    fn device_bandwidth_first_sample() {
        let mut bw = DeviceBandwidth::new();
        bw.update(1000, 2000, 5.0);
        assert_eq!(bw.bytes_rx, 1000);
        assert_eq!(bw.bytes_tx, 2000);
        // First sample: no rate calculated.
        assert_eq!(bw.rate_rx, 0.0);
        assert_eq!(bw.rate_tx, 0.0);
    }

    #[test]
    fn device_bandwidth_rolling_average() {
        let mut bw = DeviceBandwidth::new();
        bw.update(1000, 2000, 5.0);
        bw.update(2000, 4000, 5.0);

        // Second sample: drx=1000, dtx=2000 over 5s = 200 rx, 400 tx.
        assert!((bw.rate_rx - 200.0).abs() < 0.1);
        assert!((bw.rate_tx - 400.0).abs() < 0.1);
    }

    #[test]
    fn device_bandwidth_exponential_smoothing() {
        let mut bw = DeviceBandwidth::new();
        bw.update(0, 0, 5.0);
        bw.update(1000, 2000, 5.0); // instant: 200, 400
        bw.update(2000, 4000, 5.0); // instant: 200, 400

        // After smoothing: alpha=0.3
        // rate_rx = 0.3 * 200 + 0.7 * 200 = 200
        assert!((bw.rate_rx - 200.0).abs() < 0.1);
    }

    // --- Data usage ---

    #[test]
    fn data_usage_no_limit() {
        let mut tracker = DataUsageTracker::new(0);
        tracker.update(1000, 2000);
        assert!(!tracker.limit_reached());
        assert_eq!(tracker.remaining(), u64::MAX);
    }

    #[test]
    fn data_usage_limit_not_reached() {
        let mut tracker = DataUsageTracker::new(10000);
        tracker.update(3000, 2000);
        assert!(!tracker.limit_reached());
        assert_eq!(tracker.remaining(), 5000);
    }

    #[test]
    fn data_usage_limit_reached() {
        let mut tracker = DataUsageTracker::new(5000);
        tracker.update(3000, 2000);
        assert!(tracker.limit_reached());
        assert_eq!(tracker.remaining(), 0);
    }

    #[test]
    fn data_usage_limit_exceeded() {
        let mut tracker = DataUsageTracker::new(5000);
        tracker.update(4000, 3000);
        assert!(tracker.limit_reached());
        assert_eq!(tracker.remaining(), 0);
    }

    // --- Mock command runner ---

    struct MockRunner {
        responses: Arc<Mutex<HashMap<String, String>>>,
    }

    impl MockRunner {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(HashMap::new())),
            }
        }

        fn add_response(&self, key: &str, value: &str) {
            self.responses
                .lock()
                .unwrap()
                .insert(key.to_string(), value.to_string());
        }
    }

    #[async_trait::async_trait]
    impl CommandRunner for MockRunner {
        async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, ServiceError> {
            let key = format!("{} {}", command, args.join(" "));
            self.responses
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .ok_or_else(|| ServiceError::Network(format!("no mock for: {key}")))
        }
    }

    // --- Device tracker integration tests ---

    #[tokio::test]
    async fn get_connected_devices_with_mock() {
        let runner = MockRunner::new();
        runner.add_response(
            "hostapd_cli -i wlan0 all_sta",
            "00:11:22:33:44:55\naa:bb:cc:dd:ee:ff\n",
        );
        runner.add_response(
            "ip neigh",
            "192.168.4.2 dev wlan0 lladdr 00:11:22:33:44:55 REACHABLE\n",
        );

        let devices = get_connected_devices("wlan0", &runner).await.unwrap();
        assert_eq!(devices.len(), 2);

        let d1 = devices
            .iter()
            .find(|d| d.mac == "00:11:22:33:44:55")
            .unwrap();
        assert_eq!(d1.ip, "192.168.4.2");

        let d2 = devices
            .iter()
            .find(|d| d.mac == "aa:bb:cc:dd:ee:ff")
            .unwrap();
        assert!(d2.ip.is_empty());
    }

    #[tokio::test]
    async fn get_connected_devices_empty() {
        let runner = MockRunner::new();
        runner.add_response("hostapd_cli -i wlan0 all_sta", "");

        let devices = get_connected_devices("wlan0", &runner).await.unwrap();
        assert!(devices.is_empty());
    }

    #[tokio::test]
    async fn device_tracker_poll_updates() {
        let runner = MockRunner::new();
        runner.add_response("hostapd_cli -i wlan0 all_sta", "00:11:22:33:44:55\n");
        runner.add_response("ip neigh", "");

        let mut tracker = DeviceTracker::new("wlan0".to_string(), 0);
        let devices = tracker.poll(&runner).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(tracker.devices().len(), 1);
    }

    #[tokio::test]
    async fn device_tracker_removes_disconnected() {
        let runner = MockRunner::new();
        runner.add_response("hostapd_cli -i wlan0 all_sta", "00:11:22:33:44:55\n");
        runner.add_response("ip neigh", "");

        let mut tracker = DeviceTracker::new("wlan0".to_string(), 0);
        tracker.poll(&runner).await.unwrap();
        assert_eq!(tracker.devices().len(), 1);

        // Second poll: device gone.
        runner.add_response("hostapd_cli -i wlan0 all_sta", "");
        tracker.poll(&runner).await.unwrap();
        assert_eq!(tracker.devices().len(), 0);
    }
}

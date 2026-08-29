//! Data usage limits and enforcement.
//!
//! Tracks cumulative data usage, checks against configured limits,
//! and triggers auto-deactivation when the cap is reached.

use reecho_shared::DataUsage;

use crate::config::Config;

/// Threshold percentage at which a "cap approaching" warning is emitted (80%).
pub const CAP_WARNING_THRESHOLD: f64 = 0.8;

/// Data limit tracker.
pub struct DataLimitTracker {
    /// Configured data limit in bytes (0 = unlimited).
    limit: u64,
    /// Total bytes received (cumulative across all devices).
    total_rx: u64,
    /// Total bytes transmitted (cumulative across all devices).
    total_tx: u64,
    /// Timestamp when tracking started (epoch seconds).
    started_at: u64,
}

impl DataLimitTracker {
    /// Create a new tracker from config.
    pub fn from_config(config: &Config) -> Self {
        Self {
            limit: config.data_limit,
            total_rx: 0,
            total_tx: 0,
            started_at: crate::scheduler::now_epoch_secs(),
        }
    }

    /// Get the current data usage snapshot.
    pub fn usage(&self) -> DataUsage {
        DataUsage {
            total_rx: self.total_rx,
            total_tx: self.total_tx,
            limit: self.limit,
            started_at: self.started_at.to_string(),
        }
    }

    /// Get the data limit in bytes.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// Set the data limit in bytes (0 = unlimited).
    pub fn set_limit(&mut self, bytes: u64) {
        self.limit = bytes;
    }

    /// Get total bytes (rx + tx).
    pub fn total_bytes(&self) -> u64 {
        self.total_rx + self.total_tx
    }

    /// Update with new byte counters from a device poll.
    ///
    /// `new_rx` and `new_tx` are cumulative byte counters for the AP interface.
    pub fn update(&mut self, new_rx: u64, new_tx: u64) {
        self.total_rx = new_rx;
        self.total_tx = new_tx;
    }

    /// Add bytes to the cumulative counters (for incremental updates).
    pub fn add_bytes(&mut self, rx: u64, tx: u64) {
        self.total_rx = self.total_rx.saturating_add(rx);
        self.total_tx = self.total_tx.saturating_add(tx);
    }

    /// Check if the data limit is set and has been exceeded.
    pub fn is_limit_exceeded(&self) -> bool {
        self.limit > 0 && self.total_bytes() >= self.limit
    }

    /// Check if the data limit is approaching (>= 80% of cap).
    pub fn is_limit_approaching(&self) -> bool {
        if self.limit == 0 {
            return false;
        }
        let usage_pct = self.total_bytes() as f64 / self.limit as f64;
        usage_pct >= CAP_WARNING_THRESHOLD && !self.is_limit_exceeded()
    }

    /// Get the percentage of the limit used (0.0–100.0).
    /// Returns 0.0 if no limit is set.
    pub fn usage_percentage(&self) -> f64 {
        if self.limit == 0 {
            return 0.0;
        }
        (self.total_bytes() as f64 / self.limit as f64) * 100.0
    }

    /// Get the remaining bytes before the limit is reached.
    /// Returns `u64::MAX` if no limit is set.
    pub fn remaining(&self) -> u64 {
        if self.limit == 0 {
            return u64::MAX;
        }
        self.limit.saturating_sub(self.total_bytes())
    }

    /// Reset the usage counters (keeps the limit).
    pub fn reset(&mut self) {
        self.total_rx = 0;
        self.total_tx = 0;
        self.started_at = crate::scheduler::now_epoch_secs();
    }

    /// Check the limit and return the appropriate action.
    ///
    /// Returns `LimitAction` indicating what the caller should do.
    pub fn check(&self) -> LimitAction {
        if self.limit == 0 {
            return LimitAction::None;
        }
        if self.is_limit_exceeded() {
            LimitAction::Exceeded
        } else if self.is_limit_approaching() {
            LimitAction::Approaching {
                percentage: self.usage_percentage(),
            }
        } else {
            LimitAction::None
        }
    }
}

/// Action to take based on data limit check.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitAction {
    /// No limit action needed.
    None,
    /// Cap is approaching (>= 80%).
    Approaching { percentage: f64 },
    /// Cap has been reached.
    Exceeded,
}

impl std::fmt::Display for LimitAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "ok"),
            Self::Approaching { percentage } => {
                write!(f, "approaching limit ({percentage:.1}%)")
            }
            Self::Exceeded => write!(f, "limit exceeded"),
        }
    }
}

/// Format bytes to a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_with_limit() -> Config {
        Config {
            ssid: "Reecho-test".to_string(),
            password: "testpassword123".to_string(),
            band: reecho_shared::Band::Band5Ghz,
            data_limit: 1_000_000_000, // 1 GB
            blacklist: Vec::new(),
            auto_on: None,
            auto_off: None,
            schedule_repeat: "daily".to_string(),
        }
    }

    fn test_config_no_limit() -> Config {
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

    #[test]
    fn tracker_from_config_with_limit() {
        let config = test_config_with_limit();
        let tracker = DataLimitTracker::from_config(&config);
        assert_eq!(tracker.limit(), 1_000_000_000);
        assert_eq!(tracker.total_bytes(), 0);
    }

    #[test]
    fn tracker_from_config_no_limit() {
        let config = test_config_no_limit();
        let tracker = DataLimitTracker::from_config(&config);
        assert_eq!(tracker.limit(), 0);
        assert!(!tracker.is_limit_exceeded());
    }

    #[test]
    fn tracker_update_counters() {
        let config = test_config_no_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(1000, 500);
        assert_eq!(tracker.total_bytes(), 1500);
        assert_eq!(tracker.usage().total_rx, 1000);
        assert_eq!(tracker.usage().total_tx, 500);
    }

    #[test]
    fn tracker_add_bytes() {
        let config = test_config_no_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.add_bytes(100, 50);
        tracker.add_bytes(200, 75);
        assert_eq!(tracker.total_bytes(), 425);
    }

    #[test]
    fn tracker_limit_exceeded() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(600_000_000, 500_000_000);
        assert!(tracker.is_limit_exceeded());
        assert_eq!(tracker.check(), LimitAction::Exceeded);
    }

    #[test]
    fn tracker_limit_not_exceeded() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(400_000_000, 300_000_000);
        assert!(!tracker.is_limit_exceeded());
    }

    #[test]
    fn tracker_limit_approaching() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        // 850M of 1GB = 85%
        tracker.update(500_000_000, 350_000_000);
        assert!(tracker.is_limit_approaching());
        assert!(!tracker.is_limit_exceeded());
        match tracker.check() {
            LimitAction::Approaching { percentage } => {
                assert!((84.9..85.1).contains(&percentage));
            }
            _ => panic!("expected Approaching"),
        }
    }

    #[test]
    fn tracker_limit_approaching_boundary() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        // Exactly 80%
        tracker.update(400_000_000, 400_000_000);
        assert!(tracker.is_limit_approaching());
        assert!(!tracker.is_limit_exceeded());
    }

    #[test]
    fn tracker_limit_not_approaching_below_threshold() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        // 70%
        tracker.update(350_000_000, 350_000_000);
        assert!(!tracker.is_limit_approaching());
        assert_eq!(tracker.check(), LimitAction::None);
    }

    #[test]
    fn tracker_no_limit_never_exceeded() {
        let config = test_config_no_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(u64::MAX / 2, u64::MAX / 2);
        assert!(!tracker.is_limit_exceeded());
        assert!(!tracker.is_limit_approaching());
        assert_eq!(tracker.check(), LimitAction::None);
    }

    #[test]
    fn tracker_usage_percentage() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(250_000_000, 250_000_000);
        let pct = tracker.usage_percentage();
        assert!((49.9..50.1).contains(&pct));
    }

    #[test]
    fn tracker_usage_percentage_no_limit() {
        let config = test_config_no_limit();
        let tracker = DataLimitTracker::from_config(&config);
        assert_eq!(tracker.usage_percentage(), 0.0);
    }

    #[test]
    fn tracker_remaining() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(300_000_000, 100_000_000);
        let rem = tracker.remaining();
        assert_eq!(rem, 600_000_000);
    }

    #[test]
    fn tracker_remaining_no_limit() {
        let config = test_config_no_limit();
        let tracker = DataLimitTracker::from_config(&config);
        assert_eq!(tracker.remaining(), u64::MAX);
    }

    #[test]
    fn tracker_remaining_exceeded() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(600_000_000, 500_000_000);
        assert_eq!(tracker.remaining(), 0);
    }

    #[test]
    fn tracker_set_limit() {
        let config = test_config_no_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        assert_eq!(tracker.limit(), 0);
        tracker.set_limit(500_000_000);
        assert_eq!(tracker.limit(), 500_000_000);
    }

    #[test]
    fn tracker_reset() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(500_000_000, 300_000_000);
        assert!(tracker.total_bytes() > 0);
        tracker.reset();
        assert_eq!(tracker.total_bytes(), 0);
        assert_eq!(tracker.limit(), 1_000_000_000); // limit preserved
    }

    #[test]
    fn tracker_usage_snapshot() {
        let config = test_config_with_limit();
        let mut tracker = DataLimitTracker::from_config(&config);
        tracker.update(100, 200);
        let usage = tracker.usage();
        assert_eq!(usage.total_rx, 100);
        assert_eq!(usage.total_tx, 200);
        assert_eq!(usage.limit, 1_000_000_000);
    }

    #[test]
    fn limit_action_display() {
        assert_eq!(LimitAction::None.to_string(), "ok");
        assert_eq!(
            LimitAction::Approaching { percentage: 85.5 }.to_string(),
            "approaching limit (85.5%)"
        );
        assert_eq!(LimitAction::Exceeded.to_string(), "limit exceeded");
    }

    #[test]
    fn format_bytes_values() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
        assert_eq!(format_bytes(1_099_511_627_776), "1.00 TB");
    }
}

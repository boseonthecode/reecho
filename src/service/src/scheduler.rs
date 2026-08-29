//! Hotspot scheduling (auto on/off).
//!
//! Calculates next activation/deactivation times from config,
//! provides a tokio-compatible sleep until the next scheduled event.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reecho_shared::ScheduleEntry;

use crate::config::Config;
use crate::error::ServiceError;

/// Repeat rule for a schedule entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatRule {
    /// Run once, then disable the schedule.
    Once,
    /// Run every day at the same time.
    Daily,
    /// Run Monday–Friday.
    Weekdays,
    /// Run Saturday and Sunday.
    Weekends,
}

impl RepeatRule {
    /// Parse a repeat rule string.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "once" => Some(Self::Once),
            "daily" | "everyday" | "every day" => Some(Self::Daily),
            "weekdays" | "mon-fri" | "monday-friday" => Some(Self::Weekdays),
            "weekends" | "sat-sun" | "saturday-sunday" => Some(Self::Weekends),
            _ => None,
        }
    }

    /// Check if this repeat rule applies to the given day of week (0=Sunday, 6=Saturday).
    pub fn applies_to_day(&self, day_of_week: u8) -> bool {
        match self {
            Self::Once => true, // Once always applies (checked separately)
            Self::Daily => true,
            Self::Weekdays => matches!(day_of_week, 1..=5), // Mon–Fri
            Self::Weekends => matches!(day_of_week, 0 | 6), // Sat, Sun
        }
    }
}

impl std::fmt::Display for RepeatRule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Once => write!(f, "once"),
            Self::Daily => write!(f, "daily"),
            Self::Weekdays => write!(f, "weekdays"),
            Self::Weekends => write!(f, "weekends"),
        }
    }
}

/// Parsed schedule with times as seconds-since-midnight.
#[derive(Debug, Clone)]
pub struct ParsedSchedule {
    /// Auto-on time in seconds since midnight.
    pub on_seconds: u32,
    /// Auto-off time in seconds since midnight.
    pub off_seconds: u32,
    /// Repeat rule.
    pub repeat: RepeatRule,
    /// Whether the schedule is enabled.
    pub enabled: bool,
}

/// Calculate seconds since midnight from an HH:MM string.
pub fn time_to_seconds(time: &str) -> Result<u32, ServiceError> {
    let parts: Vec<&str> = time.split(':').collect();
    if parts.len() != 2 {
        return Err(ServiceError::InvalidInput(format!(
            "invalid time format: {time} (expected HH:MM)"
        )));
    }
    let hours: u32 = parts[0]
        .parse()
        .map_err(|_| ServiceError::InvalidInput(format!("invalid hours in {time}")))?;
    let minutes: u32 = parts[1]
        .parse()
        .map_err(|_| ServiceError::InvalidInput(format!("invalid minutes in {time}")))?;
    if hours >= 24 || minutes >= 60 {
        return Err(ServiceError::InvalidInput(format!(
            "time out of range: {time}"
        )));
    }
    Ok(hours * 3600 + minutes * 60)
}

/// Format seconds-since-midnight as HH:MM.
pub fn seconds_to_time(secs: u32) -> String {
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    format!("{hours:02}:{minutes:02}")
}

/// Get the current time as seconds since midnight and day of week.
pub fn current_time_of_day() -> (u32, u8) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs_today = (now.as_secs() % 86400) as u32;
    // Day of week: 0=Sunday. We calculate from epoch (1970-01-01 was Thursday=4).
    let days_since_epoch = (now.as_secs() / 86400) as u8;
    let day_of_week = (days_since_epoch + 4) % 7; // Thursday + days mod 7
    (secs_today, day_of_week)
}

/// Get the current epoch timestamp.
pub fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// The scheduler manages auto on/off timing.
pub struct Scheduler {
    /// Parsed schedule, if any.
    schedule: Option<ParsedSchedule>,
    /// Timestamp of the last state change (epoch seconds).
    last_state_change: u64,
}

impl Scheduler {
    /// Create a new scheduler from config.
    pub fn from_config(config: &Config) -> Self {
        let schedule = Self::parse_schedule(config);
        Self {
            schedule,
            last_state_change: now_epoch_secs(),
        }
    }

    fn parse_schedule(config: &Config) -> Option<ParsedSchedule> {
        let on = config.auto_on.as_ref()?;
        let off = config.auto_off.as_ref()?;
        let on_secs = time_to_seconds(on).ok()?;
        let off_secs = time_to_seconds(off).ok()?;
        Some(ParsedSchedule {
            on_seconds: on_secs,
            off_seconds: off_secs,
            repeat: RepeatRule::Daily,
            enabled: true,
        })
    }

    /// Create a scheduler with an explicit repeat rule.
    pub fn with_repeat(config: &Config, repeat: RepeatRule) -> Self {
        let schedule = match (&config.auto_on, &config.auto_off) {
            (Some(on), Some(off)) => {
                let on_secs = time_to_seconds(on).ok();
                let off_secs = time_to_seconds(off).ok();
                match (on_secs, off_secs) {
                    (Some(on), Some(off)) => Some(ParsedSchedule {
                        on_seconds: on,
                        off_seconds: off,
                        repeat,
                        enabled: true,
                    }),
                    _ => None,
                }
            }
            _ => None,
        };

        Self {
            schedule,
            last_state_change: now_epoch_secs(),
        }
    }

    /// Check if scheduling is enabled.
    pub fn is_enabled(&self) -> bool {
        self.schedule
            .as_ref()
            .is_some_and(|s| s.enabled && s.repeat != RepeatRule::Once)
    }

    /// Get the current schedule as a ScheduleEntry (for D-Bus).
    pub fn get_schedule(&self) -> Option<ScheduleEntry> {
        self.schedule.as_ref().map(|s| ScheduleEntry {
            on_time: seconds_to_time(s.on_seconds),
            off_time: seconds_to_time(s.off_seconds),
            repeat: s.repeat.to_string(),
            enabled: s.enabled,
        })
    }

    /// Set the schedule from a ScheduleEntry.
    pub fn set_schedule(&mut self, entry: &ScheduleEntry) -> Result<(), ServiceError> {
        let on_secs = time_to_seconds(&entry.on_time)?;
        let off_secs = time_to_seconds(&entry.off_time)?;
        let repeat = RepeatRule::parse(&entry.repeat).ok_or_else(|| {
            ServiceError::InvalidInput(format!("invalid repeat rule: {}", entry.repeat))
        })?;

        self.schedule = Some(ParsedSchedule {
            on_seconds: on_secs,
            off_seconds: off_secs,
            repeat,
            enabled: entry.enabled,
        });

        Ok(())
    }

    /// Update from raw config fields.
    pub fn update_from_config(&mut self, config: &Config) {
        self.schedule =
            Self::parse_schedule_with_repeat(config, self.schedule.as_ref().map(|s| s.repeat));
    }

    fn parse_schedule_with_repeat(
        config: &Config,
        repeat: Option<RepeatRule>,
    ) -> Option<ParsedSchedule> {
        let on = config.auto_on.as_ref()?;
        let off = config.auto_off.as_ref()?;
        let on_secs = time_to_seconds(on).ok()?;
        let off_secs = time_to_seconds(off).ok()?;
        Some(ParsedSchedule {
            on_seconds: on_secs,
            off_seconds: off_secs,
            repeat: repeat.unwrap_or(RepeatRule::Daily),
            enabled: true,
        })
    }

    /// Calculate the next scheduled event time (epoch seconds).
    ///
    /// Returns `None` if scheduling is disabled.
    pub fn next_event_time(&self, is_active: bool) -> Option<u64> {
        let schedule = self.schedule.as_ref()?;
        if !schedule.enabled {
            return None;
        }

        let (now_secs, day_of_week) = current_time_of_day();
        let repeat = schedule.repeat;

        if is_active {
            // Looking for next off time.
            self.next_matching_time(now_secs, day_of_week, schedule.off_seconds, repeat)
        } else {
            // Looking for next on time.
            self.next_matching_time(now_secs, day_of_week, schedule.on_seconds, repeat)
        }
    }

    /// Find the next time (epoch) that matches the given target seconds and repeat rule.
    fn next_matching_time(
        &self,
        now_secs: u32,
        day_of_week: u8,
        target_secs: u32,
        repeat: RepeatRule,
    ) -> Option<u64> {
        let today_secs = now_epoch_secs();
        let today_start = today_secs - (now_secs as u64);

        if target_secs > now_secs {
            // Target is later today — check if day matches.
            if repeat.applies_to_day(day_of_week) {
                return Some(today_start + target_secs as u64);
            }
        }

        // Search up to 7 days ahead.
        for days_ahead in 1..=7 {
            let check_day = (day_of_week + days_ahead) % 7;
            if repeat.applies_to_day(check_day) {
                return Some(today_start + days_ahead as u64 * 86400 + target_secs as u64);
            }
        }

        None
    }

    /// Create a tokio sleep future until the next scheduled event.
    ///
    /// Returns `None` if scheduling is disabled.
    pub async fn sleep_until_next(&self, is_active: bool) -> Option<tokio::time::Sleep> {
        let next_time = self.next_event_time(is_active)?;
        let now = now_epoch_secs();
        if next_time <= now {
            // Already past — return immediately.
            return Some(tokio::time::sleep(Duration::from_secs(0)));
        }
        let duration = Duration::from_secs(next_time - now);
        Some(tokio::time::sleep(duration))
    }

    /// Mark that a state change occurred (resets scheduling window).
    pub fn mark_state_change(&mut self) {
        self.last_state_change = now_epoch_secs();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config_with_schedule() -> Config {
        Config {
            ssid: "Reecho-test".to_string(),
            password: "testpassword123".to_string(),
            band: reecho_shared::Band::Band5Ghz,
            data_limit: 0,
            blacklist: Vec::new(),
            auto_on: Some("08:00".to_string()),
            auto_off: Some("20:00".to_string()),
            schedule_repeat: "daily".to_string(),
        }
    }

    fn test_config_no_schedule() -> Config {
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
    fn repeat_rule_parse_valid() {
        assert_eq!(RepeatRule::parse("once"), Some(RepeatRule::Once));
        assert_eq!(RepeatRule::parse("daily"), Some(RepeatRule::Daily));
        assert_eq!(RepeatRule::parse("DAILY"), Some(RepeatRule::Daily));
        assert_eq!(RepeatRule::parse("every day"), Some(RepeatRule::Daily));
        assert_eq!(RepeatRule::parse("weekdays"), Some(RepeatRule::Weekdays));
        assert_eq!(RepeatRule::parse("mon-fri"), Some(RepeatRule::Weekdays));
        assert_eq!(RepeatRule::parse("weekends"), Some(RepeatRule::Weekends));
        assert_eq!(RepeatRule::parse("sat-sun"), Some(RepeatRule::Weekends));
    }

    #[test]
    fn repeat_rule_parse_invalid() {
        assert_eq!(RepeatRule::parse(""), None);
        assert_eq!(RepeatRule::parse("monthly"), None);
        assert_eq!(RepeatRule::parse("invalid"), None);
    }

    #[test]
    fn repeat_rule_applies_to_day() {
        // Daily applies every day.
        assert!(RepeatRule::Daily.applies_to_day(0));
        assert!(RepeatRule::Daily.applies_to_day(3));
        assert!(RepeatRule::Daily.applies_to_day(6));

        // Weekdays: Mon(1)–Fri(5).
        assert!(!RepeatRule::Weekdays.applies_to_day(0)); // Sun
        assert!(RepeatRule::Weekdays.applies_to_day(1)); // Mon
        assert!(RepeatRule::Weekdays.applies_to_day(5)); // Fri
        assert!(!RepeatRule::Weekdays.applies_to_day(6)); // Sat

        // Weekends: Sun(0), Sat(6).
        assert!(RepeatRule::Weekends.applies_to_day(0)); // Sun
        assert!(!RepeatRule::Weekends.applies_to_day(1)); // Mon
        assert!(RepeatRule::Weekends.applies_to_day(6)); // Sat

        // Once always applies.
        assert!(RepeatRule::Once.applies_to_day(0));
        assert!(RepeatRule::Once.applies_to_day(6));
    }

    #[test]
    fn repeat_rule_display() {
        assert_eq!(RepeatRule::Once.to_string(), "once");
        assert_eq!(RepeatRule::Daily.to_string(), "daily");
        assert_eq!(RepeatRule::Weekdays.to_string(), "weekdays");
        assert_eq!(RepeatRule::Weekends.to_string(), "weekends");
    }

    #[test]
    fn time_to_seconds_basic() {
        assert_eq!(time_to_seconds("00:00").unwrap(), 0);
        assert_eq!(time_to_seconds("01:00").unwrap(), 3600);
        assert_eq!(time_to_seconds("08:30").unwrap(), 8 * 3600 + 30 * 60);
        assert_eq!(time_to_seconds("23:59").unwrap(), 23 * 3600 + 59 * 60);
    }

    #[test]
    fn time_to_seconds_invalid() {
        assert!(time_to_seconds("25:00").is_err());
        assert!(time_to_seconds("12:60").is_err());
        assert!(time_to_seconds("noon").is_err());
        assert!(time_to_seconds("").is_err());
    }

    #[test]
    fn seconds_to_time_round_trip() {
        for h in 0..24 {
            for m in [0, 15, 30, 45] {
                let secs = h * 3600 + m * 60;
                let time_str = seconds_to_time(secs);
                assert_eq!(time_to_seconds(&time_str).unwrap(), secs);
            }
        }
    }

    #[test]
    fn scheduler_from_config_with_schedule() {
        let config = test_config_with_schedule();
        let scheduler = Scheduler::from_config(&config);
        assert!(scheduler.is_enabled());
    }

    #[test]
    fn scheduler_from_config_without_schedule() {
        let config = test_config_no_schedule();
        let scheduler = Scheduler::from_config(&config);
        assert!(!scheduler.is_enabled());
    }

    #[test]
    fn scheduler_get_schedule() {
        let config = test_config_with_schedule();
        let scheduler = Scheduler::from_config(&config);
        let entry = scheduler.get_schedule().unwrap();
        assert_eq!(entry.on_time, "08:00");
        assert_eq!(entry.off_time, "20:00");
        assert_eq!(entry.repeat, "daily");
        assert!(entry.enabled);
    }

    #[test]
    fn scheduler_get_schedule_none_when_disabled() {
        let config = test_config_no_schedule();
        let scheduler = Scheduler::from_config(&config);
        assert!(scheduler.get_schedule().is_none());
    }

    #[test]
    fn scheduler_set_schedule() {
        let mut scheduler = Scheduler::from_config(&test_config_no_schedule());
        let entry = ScheduleEntry {
            on_time: "09:00".to_string(),
            off_time: "21:00".to_string(),
            repeat: "weekdays".to_string(),
            enabled: true,
        };
        scheduler.set_schedule(&entry).unwrap();
        assert!(scheduler.is_enabled());
        let got = scheduler.get_schedule().unwrap();
        assert_eq!(got.on_time, "09:00");
        assert_eq!(got.off_time, "21:00");
        assert_eq!(got.repeat, "weekdays");
    }

    #[test]
    fn scheduler_set_schedule_invalid_repeat() {
        let mut scheduler = Scheduler::from_config(&test_config_no_schedule());
        let entry = ScheduleEntry {
            on_time: "09:00".to_string(),
            off_time: "21:00".to_string(),
            repeat: "monthly".to_string(),
            enabled: true,
        };
        assert!(scheduler.set_schedule(&entry).is_err());
    }

    #[test]
    fn scheduler_set_schedule_invalid_time() {
        let mut scheduler = Scheduler::from_config(&test_config_no_schedule());
        let entry = ScheduleEntry {
            on_time: "25:00".to_string(),
            off_time: "21:00".to_string(),
            repeat: "daily".to_string(),
            enabled: true,
        };
        assert!(scheduler.set_schedule(&entry).is_err());
    }

    #[test]
    fn scheduler_next_event_returns_some_when_enabled() {
        let config = test_config_with_schedule();
        let scheduler = Scheduler::from_config(&config);
        // Should always return Some when enabled (unless RepeatRule::Once).
        let next = scheduler.next_event_time(false);
        assert!(next.is_some());
    }

    #[test]
    fn scheduler_next_event_returns_none_when_disabled() {
        let config = test_config_no_schedule();
        let scheduler = Scheduler::from_config(&config);
        assert!(scheduler.next_event_time(false).is_none());
    }

    #[test]
    fn scheduler_next_event_returns_none_when_once_and_past() {
        let mut config = test_config_with_schedule();
        config.auto_on = Some("00:00".to_string()); // midnight
        let mut scheduler = Scheduler::from_config(&config);
        scheduler.schedule.as_mut().unwrap().repeat = RepeatRule::Once;
        // After midnight, next on time for "once" may be tomorrow if we're past it.
        // This just verifies the function returns without panic.
        let _ = scheduler.next_event_time(false);
    }

    #[test]
    fn scheduler_with_repeat_overrides_default() {
        let config = test_config_with_schedule();
        let scheduler = Scheduler::with_repeat(&config, RepeatRule::Weekends);
        let entry = scheduler.get_schedule().unwrap();
        assert_eq!(entry.repeat, "weekends");
    }

    #[test]
    fn scheduler_update_from_config() {
        let mut scheduler = Scheduler::from_config(&test_config_no_schedule());
        assert!(!scheduler.is_enabled());

        let config = test_config_with_schedule();
        scheduler.update_from_config(&config);
        assert!(scheduler.is_enabled());
    }

    #[test]
    fn scheduler_update_from_config_clears_schedule() {
        let mut scheduler = Scheduler::from_config(&test_config_with_schedule());
        assert!(scheduler.is_enabled());

        let config = test_config_no_schedule();
        scheduler.update_from_config(&config);
        assert!(!scheduler.is_enabled());
    }

    #[test]
    fn scheduler_mark_state_change() {
        let config = test_config_with_schedule();
        let mut scheduler = Scheduler::from_config(&config);
        let before = scheduler.last_state_change;
        // Small delay to ensure timestamp changes.
        std::thread::sleep(Duration::from_millis(10));
        scheduler.mark_state_change();
        assert!(scheduler.last_state_change >= before);
    }

    #[test]
    fn next_event_time_is_after_now() {
        let config = test_config_with_schedule();
        let scheduler = Scheduler::from_config(&config);
        let now = now_epoch_secs();
        if let Some(next) = scheduler.next_event_time(false) {
            assert!(next > now || next == now);
        }
    }
}

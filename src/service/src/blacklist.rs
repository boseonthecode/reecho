//! Device blacklisting.
//!
//! Maintains a set of blacklisted MAC addresses and enforces blacklisting
//! by disconnecting blacklisted clients via `hostapd_cli`.

use std::collections::HashSet;

use crate::error::ServiceError;

/// Trait for running shell commands, allowing mocking.
#[async_trait::async_trait]
pub trait CommandRunner {
    async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, ServiceError>;
}

/// Device blacklist manager.
#[derive(Debug, Clone, Default)]
pub struct Blacklist {
    macs: HashSet<String>,
}

impl Blacklist {
    /// Create an empty blacklist.
    pub fn new() -> Self {
        Self {
            macs: HashSet::new(),
        }
    }

    /// Create a blacklist from an existing set of MACs.
    pub fn from_macs(macs: impl IntoIterator<Item = String>) -> Self {
        Self {
            macs: macs.into_iter().map(|m| m.to_lowercase()).collect(),
        }
    }

    /// Add a MAC address to the blacklist.
    ///
    /// Returns `true` if the MAC was newly added, `false` if already present.
    pub fn add(&mut self, mac: &str) -> bool {
        self.macs.insert(mac.to_lowercase())
    }

    /// Remove a MAC address from the blacklist.
    ///
    /// Returns `true` if the MAC was removed, `false` if not found.
    pub fn remove(&mut self, mac: &str) -> bool {
        self.macs.remove(&mac.to_lowercase())
    }

    /// Check if a MAC address is blacklisted.
    pub fn is_blacklisted(&self, mac: &str) -> bool {
        self.macs.contains(&mac.to_lowercase())
    }

    /// Get all blacklisted MAC addresses.
    pub fn macs(&self) -> Vec<String> {
        self.macs.iter().cloned().collect()
    }

    /// Get the number of blacklisted MACs.
    pub fn len(&self) -> usize {
        self.macs.len()
    }

    /// Check if the blacklist is empty.
    pub fn is_empty(&self) -> bool {
        self.macs.is_empty()
    }

    /// Filter a list of MACs, returning only those that are blacklisted.
    pub fn filter(&self, macs: &[String]) -> Vec<String> {
        macs.iter()
            .filter(|mac| self.is_blacklisted(mac))
            .cloned()
            .collect()
    }

    /// Enforce blacklisting by disconnecting blacklisted clients via hostapd_cli.
    ///
    /// Returns the list of MACs that were disconnected.
    pub async fn enforce(
        &self,
        interface: &str,
        connected_macs: &[String],
        runner: &(impl CommandRunner + Sync),
    ) -> Result<Vec<String>, ServiceError> {
        let blacklisted = self.filter(connected_macs);
        let mut disconnected = Vec::new();

        for mac in &blacklisted {
            let result = runner
                .run_command("hostapd_cli", &["-i", interface, "deauthenticate", mac])
                .await;

            match result {
                Ok(_) => {
                    tracing::info!("disconnected blacklisted device: {mac}");
                    disconnected.push(mac.clone());
                }
                Err(e) => {
                    tracing::warn!("failed to disconnect {mac}: {e}");
                }
            }
        }

        Ok(disconnected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    // --- Basic blacklist operations ---

    #[test]
    fn add_mac() {
        let mut bl = Blacklist::new();
        assert!(bl.add("AA:BB:CC:DD:EE:FF"));
        assert!(bl.is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert_eq!(bl.len(), 1);
    }

    #[test]
    fn add_mac_duplicate() {
        let mut bl = Blacklist::new();
        assert!(bl.add("AA:BB:CC:DD:EE:FF"));
        assert!(!bl.add("AA:BB:CC:DD:EE:FF"));
        assert_eq!(bl.len(), 1);
    }

    #[test]
    fn add_mac_lowercase() {
        let mut bl = Blacklist::new();
        bl.add("AA:BB:CC:DD:EE:FF");
        assert!(bl.is_blacklisted("aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn remove_mac() {
        let mut bl = Blacklist::new();
        bl.add("AA:BB:CC:DD:EE:FF");
        assert!(bl.remove("AA:BB:CC:DD:EE:FF"));
        assert!(!bl.is_blacklisted("AA:BB:CC:DD:EE:FF"));
        assert_eq!(bl.len(), 0);
    }

    #[test]
    fn remove_mac_not_found() {
        let mut bl = Blacklist::new();
        assert!(!bl.remove("AA:BB:CC:DD:EE:FF"));
    }

    #[test]
    fn from_macs() {
        let bl = Blacklist::from_macs(vec![
            "AA:BB:CC:DD:EE:FF".to_string(),
            "11:22:33:44:55:66".to_string(),
        ]);
        assert_eq!(bl.len(), 2);
        assert!(bl.is_blacklisted("aa:bb:cc:dd:ee:ff"));
    }

    #[test]
    fn macs_list() {
        let mut bl = Blacklist::new();
        bl.add("AA:BB:CC:DD:EE:FF");
        bl.add("11:22:33:44:55:66");
        let mut macs = bl.macs();
        macs.sort();
        assert_eq!(macs, vec!["11:22:33:44:55:66", "aa:bb:cc:dd:ee:ff"]);
    }

    #[test]
    fn filter_blacklisted() {
        let mut bl = Blacklist::new();
        bl.add("AA:BB:CC:DD:EE:FF");
        let connected = vec![
            "AA:BB:CC:DD:EE:FF".to_string(),
            "11:22:33:44:55:66".to_string(),
        ];
        let filtered = bl.filter(&connected);
        assert_eq!(filtered, vec!["AA:BB:CC:DD:EE:FF"]);
    }

    #[test]
    fn is_empty_check() {
        let bl = Blacklist::new();
        assert!(bl.is_empty());
    }

    // --- Enforcement tests ---

    struct MockRunner {
        responses: Arc<Mutex<Vec<String>>>,
    }

    impl MockRunner {
        fn new() -> Self {
            Self {
                responses: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn get_commands(&self) -> Vec<String> {
            self.responses.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl CommandRunner for MockRunner {
        async fn run_command(&self, command: &str, args: &[&str]) -> Result<String, ServiceError> {
            let cmd = format!("{} {}", command, args.join(" "));
            self.responses.lock().unwrap().push(cmd);
            Ok(String::new())
        }
    }

    #[tokio::test]
    async fn enforce_disconnects_blacklisted() {
        let mut bl = Blacklist::new();
        bl.add("aa:bb:cc:dd:ee:ff");

        let runner = MockRunner::new();
        let connected = vec![
            "aa:bb:cc:dd:ee:ff".to_string(),
            "11:22:33:44:55:66".to_string(),
        ];

        let disconnected = bl.enforce("wlan0", &connected, &runner).await.unwrap();
        assert_eq!(disconnected, vec!["aa:bb:cc:dd:ee:ff"]);

        let cmds = runner.get_commands();
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].contains("deauthenticate"));
        assert!(cmds[0].contains("aa:bb:cc:dd:ee:ff"));
    }

    #[tokio::test]
    async fn enforce_skips_non_blacklisted() {
        let bl = Blacklist::new(); // empty
        let runner = MockRunner::new();
        let connected = vec!["aa:bb:cc:dd:ee:ff".to_string()];

        let disconnected = bl.enforce("wlan0", &connected, &runner).await.unwrap();
        assert!(disconnected.is_empty());
        assert!(runner.get_commands().is_empty());
    }

    #[tokio::test]
    async fn enforce_disconnects_multiple() {
        let mut bl = Blacklist::new();
        bl.add("aa:bb:cc:dd:ee:ff");
        bl.add("11:22:33:44:55:66");

        let runner = MockRunner::new();
        let connected = vec![
            "aa:bb:cc:dd:ee:ff".to_string(),
            "11:22:33:44:55:66".to_string(),
            "ff:ff:ff:ff:ff:ff".to_string(),
        ];

        let mut disconnected = bl.enforce("wlan0", &connected, &runner).await.unwrap();
        disconnected.sort();
        assert_eq!(disconnected, vec!["11:22:33:44:55:66", "aa:bb:cc:dd:ee:ff"]);
        assert_eq!(runner.get_commands().len(), 2);
    }
}

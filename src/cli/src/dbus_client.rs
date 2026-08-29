//! D-Bus client for talking to the Reecho service.

use reecho_shared::{DBUS_INTERFACE, DBUS_NAME, DBUS_PATH};

/// D-Bus client connected to the Reecho service.
pub struct ReechoClient {
    proxy: zbus::proxy::Proxy<'static>,
}

impl ReechoClient {
    /// Connect to the Reecho service on the session bus.
    pub async fn connect() -> Result<Self, Box<dyn std::error::Error>> {
        let connection = zbus::Connection::session().await?;
        let proxy = zbus::proxy::Builder::new(&connection)
            .destination(DBUS_NAME)?
            .path(DBUS_PATH)?
            .interface(DBUS_INTERFACE)?
            .build()
            .await?;
        Ok(Self { proxy })
    }

    /// Get the current hotspot state.
    pub async fn get_state(&self) -> Result<String, Box<dyn std::error::Error>> {
        let state: String = self
            .proxy
            .call_method("GetState", &())
            .await?
            .body()
            .deserialize()?;
        Ok(state)
    }

    /// Get the current warning message, if any.
    pub async fn get_warning(&self) -> Result<String, Box<dyn std::error::Error>> {
        let msg: String = self
            .proxy
            .call_method("GetWarning", &())
            .await?
            .body()
            .deserialize()?;
        Ok(msg)
    }

    /// Activate the hotspot.
    pub async fn activate(
        &self,
        ssid: &str,
        password: &str,
        band: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy
            .call_method("Activate", &(ssid, password, band))
            .await?;
        Ok(())
    }

    /// Deactivate the hotspot.
    pub async fn deactivate(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy.call_method("Deactivate", &()).await?;
        Ok(())
    }

    /// Get the list of connected devices.
    pub async fn get_devices(
        &self,
    ) -> Result<Vec<(String, String, String, String, f64, f64, f64)>, Box<dyn std::error::Error>>
    {
        let devices: Vec<(String, String, String, String, f64, f64, f64)> = self
            .proxy
            .call_method("GetDevices", &())
            .await?
            .body()
            .deserialize()?;
        Ok(devices)
    }

    /// Get hotspot configuration.
    pub async fn get_config(&self) -> Result<(String, String, String), Box<dyn std::error::Error>> {
        let config: (String, String, String) = self
            .proxy
            .call_method("GetConfig", &())
            .await?
            .body()
            .deserialize()?;
        Ok(config)
    }

    /// Set hotspot configuration.
    pub async fn set_config(
        &self,
        ssid: &str,
        password: &str,
        band: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy
            .call_method("SetConfig", &(ssid, password, band))
            .await?;
        Ok(())
    }

    /// Blacklist a device by MAC address.
    pub async fn blacklist_device(&self, mac: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy.call_method("BlacklistDevice", &(mac,)).await?;
        Ok(())
    }

    /// Unblacklist a device by MAC address.
    pub async fn unblacklist_device(&self, mac: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy.call_method("UnblacklistDevice", &(mac,)).await?;
        Ok(())
    }

    /// Get cumulative data usage.
    pub async fn get_data_usage(&self) -> Result<(u64, u64, u64), Box<dyn std::error::Error>> {
        let usage: (u64, u64, u64) = self
            .proxy
            .call_method("GetDataUsage", &())
            .await?
            .body()
            .deserialize()?;
        Ok(usage)
    }

    /// Set data usage limit in bytes.
    pub async fn set_data_limit(&self, bytes: u64) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy.call_method("SetDataLimit", &(bytes,)).await?;
        Ok(())
    }

    /// Get the current auto on/off schedule.
    pub async fn get_schedule(
        &self,
    ) -> Result<(String, String, String, bool), Box<dyn std::error::Error>> {
        let schedule: (String, String, String, bool) = self
            .proxy
            .call_method("GetSchedule", &())
            .await?
            .body()
            .deserialize()?;
        Ok(schedule)
    }

    /// Set the auto on/off schedule.
    pub async fn set_schedule(
        &self,
        on_time: &str,
        off_time: &str,
        repeat: &str,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.proxy
            .call_method("SetSchedule", &(on_time, off_time, repeat, enabled))
            .await?;
        Ok(())
    }
}

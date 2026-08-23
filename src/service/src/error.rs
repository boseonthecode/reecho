//! Typed error codes for the Reecho service.

/// Service error types.
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// Configuration load/save error.
    #[error("config error: {0}")]
    Config(String),

    /// NetworkManager interaction error.
    #[error("network error: {0}")]
    Network(String),

    /// hostapd process error.
    #[error("hostapd error: {0}")]
    Hostapd(String),

    /// D-Bus communication error.
    #[error("dbus error: {0}")]
    Dbus(String),

    /// AP+STA not supported on this hardware.
    #[error("AP+STA not supported: {reason}")]
    ApStaUnsupported { reason: String },

    /// Hotspot is already in the requested state.
    #[error("hotspot is already {state}")]
    AlreadyInState { state: &'static str },

    /// Invalid input from D-Bus client.
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

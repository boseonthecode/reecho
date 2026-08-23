# Reecho

Mobile Hotspot for Linux — one toggle, your laptop becomes a hotspot.

Reecho lets a machine stay connected to Wi-Fi and broadcast a hotspot
simultaneously, sharing the same connection. It integrates into GNOME Quick
Settings as a first-class toggle.

## Features

- Toggle hotspot from GNOME Quick Settings
- Shares your current Wi-Fi connection (AP+STA concurrent mode)
- Custom SSID, password, and band selection
- Connected device list with bandwidth usage
- QR code for easy phone connection
- Scheduling and data limits
- CLI for scripting and automation

## Building

```sh
cargo build
cargo test
cargo clippy
cargo fmt --check
```

## License

MIT

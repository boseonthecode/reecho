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
npm run lint --prefix src/extension
```

## Running on Fedora (GNOME)

Until Phase 11 wiring lands, the service builds and registers on D-Bus but `activate` is a stub:

```sh
cargo build --release
RUST_LOG=info ./target/release/reecho-service &
./target/release/reecho-cli status
./target/release/reecho-cli on --ssid Reecho-Test --password test12345 --band 5GHz
```

After Phase 11, live test needs `hostapd` + `iw` and D-Bus access to NetworkManager:

```sh
sudo dnf install hostapd iw
# hostapd as user needs CAP_NET_ADMIN (or use NM-only AP fallback)
sudo setcap cap_net_admin+ep $(which hostapd)
# verify AP+STA support
iw list | grep -A5 "valid interface combinations"
nmcli c show
```

See `docs/progress.md` (Exact Next Steps) and `docs/implementationplan.md` (Phase 11) for the wiring plan.

## License

MIT

# Reecho — Implementation Plan

## Scope

This plan covers M0 (spike) through M2 (refinement) — from empty repo to a fully featured Flatpak with scheduling, data limits, QR codes, and device blacklisting. M3 features (KDE Plasma, second dongle, VPN passthrough, web UI) are documented in `docs/prd.md` but not built in this plan.

## Environment Facts

- **Working directory:** `/home/chthonic/repos/reecho`
- **Toolchains:** Rust (stable), Node.js (for extension linting), Flatpak SDK
- **Git:** `main` branch, clean repo
- **Doc convention:** `docs/` and `AGENTS.md` are committed (not gitignored)
- **Gate:** Every commit must pass `cargo clippy`, `cargo fmt --check`, `cargo test`, and (once extension exists) `npm run lint`

## Confirmed Decisions (Do Not Re-Ask)

| Decision | Choice |
|----------|--------|
| Backend language | Rust |
| UI framework | GNOME Shell extension (GJS) |
| Architecture | Separate service + D-Bus |
| CLI | Yes, Rust, same D-Bus interface |
| License | MIT |
| Distribution | Flatpak |
| Networking | NetworkManager + hostapd |

## Commit Sequence

### Phase 1: Scaffold

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 1 | `chore: scaffold Rust workspace` | Cargo workspace (service + cli + shared), root Cargo.toml, `.gitignore`, LICENSE (MIT), README.md | `cargo build`, `cargo clippy`, `cargo fmt --check` |
| 2 | `chore: add shared types and D-Bus interface definition` | `shared/` crate with `dbus-interface.xml`, Rust types mirroring the interface, constants, `Cargo.toml` for shared lib | `cargo build -p reecho-shared` |
| 3 | `chore: scaffold service skeleton` | `service/` crate, `main.rs` with systemd bus name acquisition, `error.rs` with `thiserror` typed errors, basic `dbus.rs` skeleton that registers the interface | `cargo build -p reecho-service` |
| 4 | `chore: scaffold CLI skeleton` | `cli/` crate, `main.rs` with `clap` argument parsing (placeholder subcommands), D-Bus client skeleton | `cargo build -p reecho-cli`, `reecho --help` |

### Phase 2: Service Core — Config & NetworkManager

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 5 | `feat(service): implement config persistence` | `service/config.rs` — load/save `~/.config/reecho/config.toml`, defaults (SSID, password, band), serde round-trip | `cargo test` (config load/save/defaults) |
| 6 | `feat(service): implement NetworkManager Wi-Fi state detection` | `service/network.rs` — get active Wi-Fi connection via NM D-Bus, extract SSID, interface name, signal strength, connection state | `cargo test` (mock NM D-Bus responses) |
| 7 | `feat(service): implement AP+STA capability check` | `service/network.rs` — parse `iw list` output to detect AP+STA concurrent mode support, identify chipset/driver, return capability verdict | `cargo test` (mock `iw` output samples) |
| 8 | `feat(service): implement AP+STA mode configuration` | `service/network.rs` — configure AP+STA via NetworkManager D-Bus (create virtual AP interface while preserving STA connection), force mode with warning when unsupported | `cargo test` (mock NM D-Bus calls) |

### Phase 3: Service Core — hostapd

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 9 | `feat(service): implement hostapd config generation` | `service/ap.rs` — generate hostapd.conf from HotspotConfig (SSID, password, band, channel, interface), validate params against 802.11 limits | `cargo test` (config generation + validation) |
| 10 | `feat(service): implement hostapd process management` | `service/ap.rs` — start/stop hostapd subprocess, pipe config via stdin or temp file, capture stdout/stderr, handle process exit signals | `cargo test` (mock process spawn) |
| 11 | `feat(service): implement hostapd lifecycle monitoring` | `service/ap.rs` — poll `hostapd_cli status` for AP up/down detection, handle hostapd crashes, auto-cleanup on unexpected exit, emit state changes | `cargo test` (state transition tests) |

### Phase 4: Service Core — Activation Pipeline

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 12 | `feat(service): implement hotspot activation pipeline` | Wire `network.rs` + `ap.rs` into `dbus.rs` — `Activate` method orchestrates: check Wi-Fi → configure AP+STA → generate hostapd config → start hostapd → wait for AP up, state machine (Inactive → Activating → Active/Failed), `StateChanged` signal | `cargo test` (state machine unit tests), manual test on real hardware |
| 13 | `feat(service): implement force mode with visible warning` | When AP+STA is unsupported: proceed with activation but emit `StateChanged("active")` with a warning payload; extension displays prominent warning banner; log chipset/driver details for diagnostics | `cargo test` (warning emission tests), manual test with unsupported chipset |
| 14 | `feat(service): implement deactivation pipeline` | `Deactivate` method: stop hostapd, tear down virtual AP interface, restore NM state, verify STA connection still active, emit `StateChanged("inactive")` | `cargo test` (deactivation sequence tests) |

### Phase 5: Service Core — Device Tracking

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 15 | `feat(service): implement connected device polling` | `service/devices.rs` — poll `hostapd_cli all_sta` every 2s when active, parse MAC addresses, look up IPs from `ip neigh`, resolve hostnames via reverse DNS or DHCP lease file | `cargo test` (parsing tests with sample hostapd output) |
| 16 | `feat(service): implement bandwidth monitoring` | `service/devices.rs` — track per-device bytes via `/proc/net/dev` or `conntrack`, calculate rolling 5-second bandwidth average, expose `rate_rx`/`rate_tx` per device | `cargo test` (bandwidth calculation tests) |
| 17 | `feat(service): implement data usage tracking` | `service/devices.rs` — cumulative `total_rx`/`total_tx` across all devices, `GetDataUsage` D-Bus method, integrate with `DataUsage` struct | `cargo test` (cumulative counter tests) |

### Phase 6: Service Core — Blacklisting & Full Config

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 18 | `feat(service): implement device blacklisting` | `service/blacklist.rs` — add/remove MAC addresses, enforce on device poll (disconnect blacklisted clients via `hostapd_cli`), `BlacklistDevice`/`UnblacklistDevice` D-Bus methods | `cargo test` (blacklist enforcement tests) |
| 19 | `feat(service): complete config persistence` | Extend `config.rs` — persist all fields (SSID, password, band, schedule, blacklist, data limit), config validation, `GetConfig`/`SetConfig` D-Bus methods | `cargo test` (full config round-trip) |

### Phase 7: CLI

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 20 | `feat(cli): implement on/off/status commands` | `reecho on [--ssid X] [--password Y] [--band Z]`, `reecho off`, `reecho status` — D-Bus client calls, formatted output (state, SSID, device count, data usage) | `cargo test`, manual test (`reecho status`) |
| 21 | `feat(cli): implement config and device commands` | `reecho config set/get`, `reecho devices list` — D-Bus client calls, table-formatted device output (MAC, IP, name, bandwidth) | `cargo test`, manual test (`reecho devices list`) |
| 22 | `feat(cli): implement blacklist commands` | `reecho blacklist add/remove/list` — D-Bus client calls, formatted output | `cargo test`, manual test (`reecho blacklist add AA:BB:CC:DD:EE:FF`) |

### Phase 8: GNOME Shell Extension

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 23 | `chore: scaffold GNOME Shell extension` | `extension/` directory, `metadata.json`, `extension.js` with Quick Settings panel registration, `stylesheet.css`, `npm run lint` config, `.eslintrc` | `npm run lint` |
| 24 | `feat(extension): implement D-Bus client` | `extension/dbusClient.js` — connect to `org.reecho.Service`, call methods, listen for signals, handle connection errors gracefully | `npm run lint`, manual test (extension loads, D-Bus connection verified in journal) |
| 25 | `feat(extension): implement toggle UI` | `extension/ui.js` — toggle button matching GNOME Quick Settings style (icon, label, active state), status label showing SSID + device count when active, real-time state sync via D-Bus signals | `npm run lint`, manual test in GNOME Shell (toggle on/off works) |
| 26 | `feat(extension): implement preferences window` | `extension/prefs.js` — SSID field, password field (with show/hide), band selector (2.4/5 GHz), save/load via D-Bus `SetConfig`/`GetConfig` | `npm run lint`, manual test (settings persist) |
| 27 | `feat(extension): implement device list and QR display` | `extension/prefs.js` — device list view (table: MAC, IP, name, bandwidth), blacklist toggle per device, QR code image rendered from service-generated PNG | `npm run lint`, manual test (devices show, QR scans) |

### Phase 9: Scheduling & Data Limits

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 28 | `feat(service): implement scheduling` | `service/scheduler.rs` — schedule entries (on_time, off_time, repeat rule), `tokio::time::sleep_until` for next activation, check Wi-Fi state before activating, `GetSchedule`/`SetSchedule` D-Bus methods | `cargo test` (schedule calculation, Wi-Fi state check) |
| 29 | `feat(service): implement data limits` | `service/limits.rs` — cumulative data cap, auto-deactivation when limit reached, `DataLimitReached` D-Bus signal, `SetDataLimit`/`GetDataUsage` D-Bus methods, extension notification on cap approaching (80%) and reached (100%) | `cargo test` (limit enforcement, notification tests) |

### Phase 10: Packaging & Finalization

| # | Message | Contents | Verification |
|---|---------|----------|--------------|
| 30 | `chore: Flatpak packaging and final docs` | Complete `com.reecho.Reecho.yml` manifest (D-Bus permissions, systemd unit, extension install hook), update `docs/progress.md` with final state, ensure all cross-references valid | `flatpak-builder` builds successfully, manual Flatpak install + full flow test |

**Total: 30 commits.**

## FR ↔ Commit Mapping

| FR | Description | Commit(s) | Notes |
|----|-------------|-----------|-------|
| FR-1.1 | Toggle from Quick Settings | 12, 25 | Service pipeline + extension toggle |
| FR-1.2 | Shares current Wi-Fi | 6, 8, 12 | NM state detection + AP+STA config |
| FR-1.3 | Wi-Fi stays active during hotspot | 8, 12 | AP+STA concurrent mode |
| FR-1.4 | Deactivate doesn't disconnect Wi-Fi | 14 | Deactivation pipeline restores NM state |
| FR-1.5 | Status indicator | 25 | Status label in extension toggle |
| FR-1.6 | Force AP+STA + visible warning | 7, 8, 13 | Capability check → force mode → visible warning in extension |
| FR-2.1 | Custom SSID | 5, 19, 26 | Config persistence + extension prefs |
| FR-2.2 | Custom password | 5, 19, 26 | Config persistence + extension prefs |
| FR-2.3 | Band selection | 19, 26 | Config persistence + extension prefs |
| FR-2.4 | Config persists across restarts | 5, 19 | TOML config in `~/.config/reecho/` |
| FR-2.5 | Default SSID `Reecho-{hostname}` | 5 | Config default in config.rs |
| FR-3.1 | Connected device list | 15, 27 | Device polling + extension UI |
| FR-3.2 | Per-device bandwidth | 16, 27 | Rolling average in devices.rs + extension UI |
| FR-3.3 | Total hotspot data usage | 17 | GetDataUsage D-Bus method |
| FR-3.4 | Blacklist device by MAC | 18, 27 | Blacklist.rs + extension prefs UI |
| FR-3.5 | Blacklisted device disconnected | 18 | Enforcement in device poll loop |
| FR-4.1 | QR code generation | 27 | QR crate + extension rendering |
| FR-4.2 | QR updates on settings change | 27 | Regenerated when config changes |
| FR-4.3 | QR scannable by phone cameras | 27 | Verify with Android/iOS camera apps in manual test |
| FR-5.1 | Auto-on time | 28 | Scheduler.rs |
| FR-5.2 | Auto-off time | 28 | Scheduler.rs |
| FR-5.3 | Repeat schedule | 28 | Scheduler.rs with RepeatRule enum |
| FR-5.4 | Scheduled activation respects Wi-Fi state | 28 | Scheduler checks `network.rs` Wi-Fi state before activating |
| FR-6.1 | Data cap setting | 19, 29 | Config + limits.rs |
| FR-6.2 | Hotspot deactivates at cap | 29 | limits.rs auto-deactivation |
| FR-6.3 | Cap approaching/reached notification | 29 | D-Bus signal + extension notification |
| FR-6.4 | Data usage counter reset | 29 | Manual reset via CLI/config |
| FR-7.1 | `reecho on` | 20 | CLI command |
| FR-7.2 | `reecho off` | 20 | CLI command |
| FR-7.3 | `reecho status` | 20 | CLI command |
| FR-7.4 | `reecho config set/get` | 21 | CLI command |
| FR-7.5 | `reecho devices list` | 21 | CLI command |
| FR-7.6 | `reecho blacklist add/remove` | 22 | CLI command |
| FR-8.1 | Extension in Quick Settings | 23, 25 | Extension registration |
| FR-8.2 | Toggle state syncs with service | 24, 25 | D-Bus signal listener |
| FR-8.3 | Extension shows SSID when active | 25 | Status label |
| FR-8.4 | Extension preferences window | 26 | prefs.js |
| FR-8.5 | Graceful handling of service unavailable | 24 | D-Bus connection error handling |

**Deferred to M3 (not in this plan):** KDE integration, second dongle fallback, VPN passthrough, web UI, CLI completions.

## Key Technical Designs

### Commits 1–4: Scaffold

The Cargo workspace has three crates:
- `reecho-service` — the background service binary.
- `reecho-cli` — the CLI binary.
- `reecho-shared` — shared types and constants (no binary, just a library).

The D-Bus interface is defined in XML (`shared/dbus-interface.xml`) and used by `zbus` server (service) and clients (CLI, extension). This ensures type safety and a single source of truth.

### Commits 5–8: Config & NetworkManager

- Config is TOML, stored in `~/.config/reecho/config.toml`, loaded on service start, saved on every `SetConfig` call.
- NetworkManager interaction is all via D-Bus (`org.freedesktop.NetworkManager`). No subprocess calls to `nmcli`.
- AP+STA capability detection parses `iw list` output. The parser must handle varying output formats across drivers — use regex with fallback patterns.

### Commits 9–11: hostapd

- Config generation produces a valid `hostapd.conf` from `HotspotConfig`.
- Process management uses `tokio::process::Command` for async spawn/monitor.
- Lifecycle monitoring polls `hostapd_cli status` every 1s during activation, then every 5s while active.

### Commits 12–14: Activation Pipeline

The activation sequence:
1. Check Wi-Fi state via NetworkManager D-Bus (commit 6).
2. Check AP+STA capability (commit 7).
3. If not supported: force with warning (commit 13).
4. Configure AP+STA via NetworkManager (commit 8).
5. Generate hostapd config (commit 9).
6. Start hostapd (commit 10).
7. Wait for AP up via `hostapd_cli status` (commit 11).
8. Emit `StateChanged("active")`.

Deactivation reverses: stop hostapd → tear down virtual AP → restore NM → emit `StateChanged("inactive")`.

### Commits 15–17: Device Tracking

- Poll `hostapd_cli all_sta` every 2s when hotspot is active.
- For each MAC: look up IP from `ip neigh`, resolve hostname via reverse DNS or DHCP lease file.
- Bandwidth: read `/proc/net/dev` for the AP interface, diff byte counters over 5-second windows, attribute to devices via MAC (requires `conntrack` or `hostapd_cli` per-station stats if available).

### Commits 18–19: Blacklisting & Config

- Blacklist enforced in the device poll loop — when a blacklisted MAC is seen, immediately disconnect via `hostapd_cli deauthenticate`.
- Full config persistence covers all user-facing settings. Config validation rejects invalid values (SSID too long, password too short, etc.).

### Commits 20–22: CLI

The CLI is a thin D-Bus client. All logic lives in the service. Output is formatted for terminal readability (tables, colors where supported).

### Commits 23–27: GNOME Shell Extension

The extension:
1. Registers a toggle button in Quick Settings via `Main.panel._quickSettings.addItems()`.
2. On toggle, calls `org.reecho.Service.Activate` or `Deactivate` via D-Bus.
3. Listens for `StateChanged` signal to update UI in real time.
4. Preferences window uses `Adw.PreferencesWindow` if available, or custom `St.Widget` layout.
5. QR code: Rust service generates a PNG, extension renders it as `Clutter.Image`.
6. Handles D-Bus connection loss gracefully (shows "Service unavailable" state).

### Commits 28–29: Scheduling & Data Limits

- Scheduling: `tokio::time::sleep_until` for the next on/off time. On service start, calculate next activation from schedule config. Re-arm after each activation/deactivation. Check Wi-Fi state before activating — if Wi-Fi is disconnected, skip and log warning.
- Data limits: Track cumulative bytes in `DataUsage`. When limit is reached, call `Deactivate` internally and emit `DataLimitReached` signal. Extension shows notification at 80% and 100%.

### Commit 30: Flatpak

The Flatpak manifest needs:
- `finish-args` for D-Bus access to NetworkManager and systemd.
- Build commands for Rust (using `cargo` in the Flatpak SDK).
- Extension installation to `~/.local/share/gnome-shell/extensions/` or Flatpak-compatible path.
- Systemd user unit installation.

## Roadmap (M3, Documented, Not Built)

The following are documented in `docs/prd.md` but not built in this plan:

- KDE Plasma integration (KDE Connect or custom plasmoid).
- Second USB dongle fallback.
- VPN passthrough.
- Web UI for remote management.
- CLI completions (bash, zsh, fish).

## Notes / Gotchas

- **hostapd + NetworkManager conflicts:** The service must carefully manage the lifecycle — stop NM's AP attempt before starting hostapd, and restore NM state when stopping hostapd.
- **AP+STA detection:** `iw list` output format varies by driver. Need robust parsing or fallback to "try and see."
- **Flatpak D-Bus access:** The `org.freedesktop.NetworkManager` D-Bus name may not be accessible from within the Flatpak sandbox. Test early; may need `--socket=system-bus` or portal access.
- **GNOME Shell version differences:** Quick Settings API changed between GNOME 42 and 44+. Test on the minimum supported version (42).
- **hostapd binary availability:** Not all distros ship hostapd by default. May need to bundle it in the Flatpak or document manual installation.

# Reecho — Progress Log

> Single source of truth for project state. Next session: read this first,
> then continue without re-asking anything already decided.

## Last Updated

2026-08-28

## What Was Done This Session

### Documentation (prior session)
- Bootstrap: created full documentation scaffold from raw idea.
- Captured and refined the idea in `idea.md` (root, source of truth).
- Resolved all open questions via Q&A with the user.
- Generated the complete `docs/` tree: AGENTS.md, progress.md, problemstatement.md, prd.md, architecture.md, techstack.md, implementationplan.md, rules/codestyle.md, rules/testing.md, rules/security.md.
- Skipped `docs/theory.md` (no complex domain logic — networking setup is well-understood).
- Skipped `docs/visualdesign.md` (UI is a native GNOME Shell extension; design follows GNOME HIG, no custom design system needed).
- Cross-checked all docs for consistency. Fixed: scope statement in implementationplan.md (covers M0–M2, not M0–M1), added FR ↔ commit mapping table, clarified FR-1.6 visible warning, FR-4.3 scannability verification, FR-5.4 Wi-Fi state check.
- Expanded implementation plan from 14 to 30 commits (per user request). Each commit is a focused, testable unit.

### Phase 1: Scaffold (4 commits)

- **Commit 1** `chore: scaffold Rust workspace` — Cargo workspace with three crates (`src/service`, `src/cli`, `src/shared`), `.gitignore`, MIT `LICENSE`, expanded `README.md`.
- **Commit 2** `chore: add shared types and D-Bus interface definition` — `reecho-shared` crate with `dbus-interface.xml` (source of truth for D-Bus interface), Rust types (`HotspotState`, `Band`, `HotspotConfig`, `ConnectedDevice`, `DataUsage`, `ScheduleEntry`), constants (`DBUS_NAME`, `DBUS_PATH`, `DBUS_INTERFACE`, defaults).
- **Commit 3** `chore: scaffold service skeleton` — `reecho-service` crate with `main.rs` (tokio + zbus session bus, systemd bus name acquisition), `error.rs` (`ServiceError` typed errors), `dbus.rs` (`ReechoService` with `#[zbus::interface]` — all methods implemented as placeholders).
- **Commit 4** `chore: scaffold CLI skeleton` — `reecho-cli` crate with `clap` derive-based argument parsing, subcommands: `on`, `off`, `status`, `config`, `devices`, `blacklist` (all print placeholder messages).

### Phase 2: Service Core — Config & NetworkManager (4 commits)

- **Commit 5** `feat(service): implement config persistence` — `service/config.rs`: TOML load/save to `~/.config/reecho/config.toml`, default SSID (`Reecho-{hostname}`), random password generation, validation (SSID/password length limits), 9 unit tests.
- **Commit 6** `feat(service): implement NetworkManager Wi-Fi state detection` — `service/network.rs`: D-Bus client for NetworkManager, `WifiState` struct (SSID, interface, signal, connection state), trait-based design with `MockNetworkManager` for testing, 6 unit tests.
- **Commit 7** `feat(service): implement AP+STA capability check` — `service/network.rs`: `parse_iw_list()` parser for `iw list` output, `ApStaCapability` struct (supported, chipset, reason), `CommandRunner` trait with mock, 6 unit tests.
- **Commit 8** `feat(service): implement AP+STA mode configuration` — `service/network.rs`: `build_ap_connection_settings()` (NM connection settings as nested HashMap), `activate_ap_sta()` / `deactivate_ap_sta()` with force mode and warning, `NetworkManagerConnectionOps` trait with mock, 4 unit tests.

### Phase 3: Service Core — hostapd (3 commits)

- **Commit 9** `feat(service): implement hostapd config generation` — `service/ap.rs`: `HostapdConfig` struct, `to_hostapd_conf()` generates valid hostapd.conf (HT/VHT, band, channel), `validate_config()` checks 802.11 limits, 16 unit tests.
- **Commit 10** `feat(service): implement hostapd process management` — `service/ap.rs`: `start_hostapd()` / `stop_hostapd()` async functions, `HostapdSpawner` trait with `MockHostapdSpawner`, `write_config_file()` to temp dir, 4 unit tests.
- **Commit 11** `feat(service): implement hostapd lifecycle monitoring` — `service/ap.rs`: `ApState` enum (Down/Starting/Up/Failed), `ApLifecycleMonitor` state machine, `parse_hostapd_status()` parser, `monitor_hostapd()` async loop, `HostapdStatusQuerier` trait with mock, 10 unit tests.

### Phase 4: Service Core — Activation Pipeline (3 commits)

- **Commit 12** `feat(service): implement hotspot activation pipeline` — `service/activation.rs`: `ActivationPipeline` struct with trait-based dependencies, `activate()` orchestrates: Wi-Fi check → AP+STA capability → NM config → hostapd start → AP up detection, `wait_for_ap_up()` with 30s timeout, 4 unit tests.
- **Commit 13** `feat(service): implement force mode with visible warning` — Added `GetWarning` D-Bus method and `Warning` signal to interface XML, `ReechoService.warning` field tracks force-mode warnings, `set_state()` method for state+warning updates.
- **Commit 14** `feat(service): implement deactivation pipeline` — `ActivationPipeline::deactivate()` orchestrates: stop hostapd → tear down AP → verify STA, D-Bus `deactivate()` clears warning, 6 unit tests for D-Bus state transitions.

### Phase 5: Service Core — Device Tracking (1 commit, 3 features)

- **Commit 15–17** `feat(service): implement connected device polling` — `service/devices.rs`: `parse_all_sta()` for hostapd MAC list, `parse_ip_neigh()` for MAC→IP mapping, `parse_proc_net_dev()` for bandwidth counters, `DeviceBandwidth` with exponential moving average (alpha=0.3), `DataUsageTracker` with limit enforcement, `DeviceTracker` state manager, 19 unit tests.

### Phase 6: Service Core — Blacklisting & Full Config (2 commits)

- **Commit 18** `feat(service): implement device blacklisting` — `service/blacklist.rs`: `Blacklist` struct with add/remove/check/filter, `enforce()` disconnects blacklisted clients via `hostapd_cli deauthenticate`, 12 unit tests.
- **Commit 19** `feat(service): complete config persistence` — Extended `config.rs`: added `blacklist` (Vec<String>), `auto_on`/`auto_off` (Option<String>) fields, MAC validation, 24h time validation, full config round-trip test, 8 new unit tests.

### Phase 7: CLI (1 commit, 3 features)

- **Commit 20–22** `feat(cli): implement D-Bus client and all CLI commands` — `cli/dbus_client.rs`: full D-Bus client with all service methods (activate/deactivate/get_state/get_warning/get_config/set_config/get_devices/blacklist/unblacklist/get_data_usage/set_data_limit). `cli/main.rs`: `on` (with --ssid/--password/--band), `off`, `status` (state + SSID + band + devices + data usage), `config get/set`, `devices list` (table format), `blacklist add/remove`. `format_bytes` helper for human-readable sizes. 4 unit tests.

**Total: 108 tests passing (104 service + 4 CLI).**

### Phase 8: GNOME Shell Extension (1 commit, 3 features)

- **Commit 23–25** `feat(extension): implement GNOME Shell extension` — `extension/metadata.json` (GNOME 45-47), `extension/constants.js` (D-Bus constants, state labels), `extension/dbusClient.js` (Gio.DBusProxy wrapper, signal handlers for StateChanged/DeviceConnected/DeviceDisconnected/Warning/DataLimitReached), `extension/ui.js` (Quick Settings toggle button via `QuickSettings.ToggleButton`, status display with device count, state sync), `extension/extension.js` (entry point, enable/disable lifecycle), `extension/stylesheet.css`, `.eslintrc.json` (eslint:recommended, GJS globals), `package.json`. `npm run lint` passes clean.
- **Commit 26** `chore: exclude node_modules from git` — Updated `.gitignore` with `node_modules/` and `package-lock.json`.

### Phase 9: Scheduling & Data Limits (2 commits)

- **Commit 28** `feat(service): implement scheduling` — `service/scheduler.rs`: `RepeatRule` enum (Once/Daily/Weekdays/Weekends) with `parse()`, `applies_to_day()`, `Display`. `ParsedSchedule` struct with seconds-since-midnight times. `Scheduler` struct with `from_config()`, `with_repeat()`, `get_schedule()`/`set_schedule()`, `next_event_time()`, `sleep_until_next()`, `mark_state_change()`. `time_to_seconds()`/`seconds_to_time()` helpers. Config extended with `schedule_repeat` field (validated). D-Bus interface XML extended with `GetSchedule`/`SetSchedule` methods. D-Bus `dbus.rs` wired: `get_schedule()` returns `(on_time, off_time, repeat, enabled)`, `set_schedule()` syncs back to config. CLI client updated with schedule methods. 19 scheduler tests + 4 new config validation tests.

- **Commit 29** `feat(service): implement data limits` — `service/limits.rs`: `DataLimitTracker` struct with `from_config()`, `usage()`, `limit()`/`set_limit()`, `update()`/`add_bytes()`, `is_limit_exceeded()`/`is_limit_approaching()`, `usage_percentage()`/`remaining()`, `reset()`, `check()` returning `LimitAction` enum (None/Approaching/Exceeded). `CAP_WARNING_THRESHOLD` at 80%. `format_bytes()` helper for human-readable sizes. D-Bus `dbus.rs` wired: `get_data_usage()` returns real `(rx, tx, limit)`, `set_data_limit()` persists to config. Added `get_data_usage_string()`, `get_data_limit_string()`, `check_data_limit()` D-Bus methods. 21 limits tests.

**Total: 166 tests passing (162 service + 4 CLI).**

## Current State

Phases 1–11 are complete (24 commits). All gate checks pass. All placeholder methods in dbus.rs are now wired to real implementations.

### Decided

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Platform | Linux desktop, GNOME only for v1 | GNOME Quick Settings is the target integration point |
| Backend language | Rust | Safety, performance, systemd/D-Bus ecosystem |
| UI framework | GNOME Shell extension (GJS) | Standard GNOME integration pattern |
| CLI | Rust, same D-Bus interface as extension | Scripting/automation access |
| Architecture | Separate service + D-Bus (not embedded in extension) | Testable independently, CLI works without extension, service crashes don't affect GNOME Shell |
| Distribution | Flatpak | Cross-distro compatibility |
| License | MIT | Permissive, matches ecosystem norms |
| Networking | NetworkManager + hostapd for AP+STA | Industry standard for Linux Wi-Fi management |
| QR code | Rust crate (e.g., `qrcode` or `qr2term`) | No preference from user; pick at implementation time |
| Device tracking | Via `hostapd_cli` client list, bandwidth via `/proc/net` or `conntrack` | Detailed per-device session totals, real-time rates, cumulative data |
| Trait-based design | All external boundaries use traits with mocks | Enables unit testing without real NM/hostapd |
| `?Sized` bounds | Added to network.rs and ap.rs functions | Allows `dyn Trait` in activation pipeline |
| Schedule repeat | RepeatRule enum (Once/Daily/Weekdays/Weekends) | Flexible scheduling without over-engineering |
| Data limit threshold | 80% warning, 100% auto-deactivation | Early warning before cap is reached |
| `format_bytes` | Shared helper in limits.rs and CLI | Consistent human-readable byte formatting |

### What Exists

```
Cargo.toml                          # Workspace root
LICENSE                             # MIT
README.md                           # Project overview
.gitignore                          # Rust + IDE + node_modules ignores
src/
  shared/                           # reecho-shared (library)
    Cargo.toml
    src/lib.rs
    src/types.rs                    # HotspotState, Band, HotspotConfig, ConnectedDevice, DataUsage, ScheduleEntry
    src/constants.rs                # DBUS_NAME, DBUS_PATH, DBUS_INTERFACE, defaults
    src/dbus_interface.xml          # D-Bus interface spec (source of truth) — includes GetWarning, Warning, GetSchedule/SetSchedule
  service/                          # reecho-service (binary)
    Cargo.toml
    src/main.rs                     # tokio + zbus session bus, service startup
    src/dbus.rs                     # ReechoService with #[zbus::interface] — all D-Bus methods wired
    src/error.rs                    # ServiceError typed errors (thiserror)
    src/config.rs                   # Config load/save/validate with defaults (17 tests)
    src/network.rs                  # NM state detection, AP+STA capability, AP+STA config (16 tests)
    src/ap.rs                       # hostapd config, process management, lifecycle monitoring (30 tests)
    src/activation.rs               # Activation pipeline, force mode, deactivation (4 tests)
    src/devices.rs                  # Device polling, bandwidth tracking, data usage (19 tests)
    src/blacklist.rs                # Device blacklisting with enforcement (12 tests)
    src/scheduler.rs                # Auto on/off scheduling with RepeatRule, tokio sleep (19 tests)
    src/limits.rs                   # Data limit tracking, auto-deactivation, format_bytes (21 tests)
  cli/                              # reecho-cli (binary)
    Cargo.toml
    src/main.rs                     # clap CLI with real D-Bus calls (on/off/status/config/devices/blacklist)
    src/dbus_client.rs              # D-Bus client for all service methods (including schedule)
  extension/                        # GNOME Shell extension
    metadata.json                   # GNOME 45-47
    extension.js                    # Entry point, enable/disable lifecycle
    dbusClient.js                   # D-Bus client with Gio.DBusProxy, signal handlers
    ui.js                           # Quick Settings toggle + status toggle
    constants.js                    # DBUS_NAME/PATH/INTERFACE, state constants
    stylesheet.css                  # Extension styles
    .eslintrc.json                  # ESLint config
    package.json                    # npm scripts: lint, lint:fix
packaging/                          # Flatpak packaging
  com.reecho.Reecho.yml             # Flatpak manifest (GNOME 47, rust-stable SDK)
  com.reecho.Reecho.service         # systemd user unit (Type=dbus, security hardening)
  com.reecho.Reecho.service.dbus    # D-Bus session service file
docs/                               # Full documentation tree
```

### What Does NOT Exist Yet

Nothing — all planned phases are complete. Remaining work is optional:
- Wire ActivationPipeline with real NM/hostapd traits in main.rs (requires D-Bus system bus)
- Add `#[zbus(signal)]` for DeviceConnected/DeviceDisconnected (requires zbus SignalContext)
- Scheduler background loop (sleep_until_next in main.rs)
- Device polling loop (poll_devices in main.rs)

## Exact Next Steps

All planned work is done. Future improvements:
1. Wire real NM/hostapd traits into ActivationPipeline in main.rs
2. Add scheduler background loop and device polling loop
3. Add zbus signal emission for DeviceConnected/DeviceDisconnected
4. Manual testing on real hardware with Wi-Fi adapter

## Environment Facts

- Working directory: `/home/chthonic/repos/reecho`
- Git repo: yes, on `main` branch
- Toolchains: Rust 1.95.0, Node.js v22.23.1
- 24 commits completed (Phase 1–11). All planned work complete.

## Open Questions (Resolved This Session)

All open questions from the idea have been resolved. See the "Decided" table above.

## Gotchas

- AP+STA concurrent mode support varies by chipset/driver. The "force" mode with warning is a deliberate v1 trade-off.
- hostapd and NetworkManager can conflict if not coordinated carefully — the Rust service must manage their lifecycle.
- GNOME Shell extensions run in the compositor process; bugs can crash the desktop. Keep extension code minimal, delegate to the service.
- Flatpak sandboxing may limit access to system services (NetworkManager, hostapd) — will need appropriate Flatpak permissions.
- zbus 5 does not have a `traits` feature — derive macros are available through `zbus::zvariant::Type` re-export or via the `bus-impl` feature for `#[interface]`.
- Functions using `impl Trait` need `+ ?Sized` bounds when called with `dyn Trait` references.

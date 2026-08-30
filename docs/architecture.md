# Reecho — Architecture

## 1. Architectural Goals & Constraints

| Goal | Architectural Consequence |
|------|--------------------------|
| GNOME-native feel | Extension integrates into Quick Settings panel, not a standalone app |
| Cross-distro | Flatpak packaging; no distro-specific dependencies |
| Reliability | Service runs as systemd user unit with auto-restart; extension is thin |
| Testability | Business logic in Rust service; extension delegates via D-Bus |
| Security | User-level service only; no root; no outbound network; D-Bus interface validated |
| Maintainability | Shared interface definitions; clean separation between service, CLI, extension |

**Constraints:**
- Linux only (GNOME for v1).
- NetworkManager must be running (standard on all target distros).
- hostapd required for AP mode.
- Flatpak sandbox may limit D-Bus access — permissions must be configured.
- AP+STA concurrent mode depends on Wi-Fi chipset/driver support.

## 2. System Context

```
┌─────────────────────────────────────────────────────────┐
│                     User's Machine                       │
│                                                         │
│  ┌──────────────┐     D-Bus      ┌──────────────────┐  │
│  │   GNOME Shell │◄─────────────►│  reecho-service   │  │
│  │   Extension   │               │  (Rust, systemd)  │  │
│  └──────────────┘                └────────┬─────────┘  │
│                                           │             │
│  ┌──────────────┐                ┌────────▼─────────┐  │
│  │  reecho CLI   │──── D-Bus ───►│  NetworkManager   │  │
│  │  (Rust)       │               │  (system D-Bus)   │  │
│  └──────────────┘                └────────┬─────────┘  │
│                                           │             │
│                                  ┌────────▼─────────┐  │
│                                  │     hostapd       │  │
│                                  │  (process/D-Bus)  │  │
│                                  └──────────────────┘  │
│                                                         │
└─────────────────────────────────────────────────────────┘
         │                                  │
         ▼                                  ▼
   ┌──────────┐                    ┌──────────────┐
   │  Client   │                    │   Internet   │
   │  Devices  │◄──── Wi-Fi ───────│  (via Wi-Fi) │
   │ (phone,   │     Hotspot       │              │
   │  tablet)  │                    └──────────────┘
   └──────────┘
```

## 3. Top-Level Components

| Component | Responsibility | Where It Runs |
|-----------|---------------|---------------|
| `reecho-service` | Hotspot lifecycle, NetworkManager/hostapd orchestration, device tracking, scheduling, data limits | User's machine, systemd user unit |
| `reecho-extension` | Quick Settings toggle, device list UI, QR display, preferences | GNOME Shell process (extension) |
| `reecho-cli` | Command-line interface for scripting/automation | User's terminal, talks to service via D-Bus |
| `shared` | D-Bus interface definitions, constants | Compiled into service, CLI, and extension |

## 4. Client/Module Architecture

```
src/
  service/          # Rust background service
    main.rs         # Entry point, systemd integration
    dbus.rs         # D-Bus interface handlers
    network.rs      # NetworkManager interaction
    ap.rs           # hostapd management
    devices.rs      # Device tracking, bandwidth monitoring
    scheduler.rs    # Scheduling logic
    limits.rs       # Data cap enforcement
    blacklist.rs    # Device blacklisting
    config.rs       # Configuration persistence
    error.rs        # Typed error codes

  cli/              # Rust CLI tool
    main.rs         # Argument parsing, D-Bus client calls
    commands.rs     # Command implementations

  extension/        # GNOME Shell extension (GJS)
    extension.js    # Entry point, Quick Settings panel registration
    ui.js           # Panel UI components
    dbusClient.js   # D-Bus client wrapper
    qr.js           # QR code rendering
    prefs.js        # Preferences window
    stylesheet.css  # Extension styles

  shared/           # Shared definitions
    dbus-interface.xml  # D-Bus interface spec (source of truth)
    constants.rs        # Rust constants
    constants.js        # JS constants (mirrored from Rust)
```

**Hard rule:** `service/` and `cli/` must not import any GNOME/extension code. `extension/` must not contain business logic — it delegates to the service via D-Bus. `shared/` contains only data definitions.

## 5. Module ↔ Thread/Process Protocol

All communication between components uses D-Bus (user session bus).

### D-Bus Interface: `org.reecho.Service`

```
<node>
  <interface name="org.reecho.Service">
    <method name="GetState">
      <arg name="state" type="s" direction="out"/>
    </method>
    <method name="Activate">
      <arg name="ssid" type="s" direction="in"/>
      <arg name="password" type="s" direction="in"/>
      <arg name="band" type="s" direction="in"/>
    </method>
    <method name="Deactivate"/>
    <method name="GetDevices">
      <arg name="devices" type="a(ssssddd)" direction="out"/>
    </method>
    <method name="BlacklistDevice">
      <arg name="mac" type="s" direction="in"/>
    </method>
    <method name="UnblacklistDevice">
      <arg name="mac" type="s" direction="in"/>
    </method>
    <method name="GetDataUsage">
      <arg name="rx" type="t" direction="out"/>
      <arg name="tx" type="t" direction="out"/>
      <arg name="limit" type="t" direction="out"/>
    </method>
    <method name="SetDataLimit">
      <arg name="bytes" type="t" direction="in"/>
    </method>
    <method name="GetConfig">
      <arg name="ssid" type="s" direction="out"/>
      <arg name="password" type="s" direction="out"/>
      <arg name="band" type="s" direction="out"/>
    </method>
    <method name="SetConfig">
      <arg name="ssid" type="s" direction="in"/>
      <arg name="password" type="s" direction="in"/>
      <arg name="band" type="s" direction="in"/>
    </method>
    <signal name="StateChanged">
      <arg name="state" type="s"/>
    </signal>
    <signal name="DeviceConnected">
      <arg name="mac" type="s"/>
      <arg name="name" type="s"/>
    </signal>
    <signal name="DeviceDisconnected">
      <arg name="mac" type="s"/>
    </signal>
    <signal name="DataLimitReached"/>
  </interface>
</node>
```

## 6. Core Pipeline

### Hotspot Activation Flow

```
User toggles ON
       │
       ▼
┌─────────────┐
│  Extension   │──D-Bus──►┌──────────────┐
│  (toggle UI) │          │   Service     │
└─────────────┘          │  (dbus.rs)    │
                         └──────┬───────┘
                                │
                    ┌───────────▼───────────┐
                    │  network.rs            │
                    │  1. Check Wi-Fi state  │
                    │  2. Get current SSID   │
                    │  3. Configure AP+STA   │
                    └───────────┬───────────┘
                                │
                    ┌───────────▼───────────┐
                    │  ap.rs                 │
                    │  1. Generate hostapd   │
                    │     config             │
                    │  2. Start hostapd      │
                    │  3. Wait for AP up     │
                    └───────────┬───────────┘
                                │
                    ┌───────────▼───────────┐
                    │  devices.rs            │
                    │  Start device polling  │
                    │  (hostapd_cli all_sta) │
                    └───────────┬───────────┘
                                │
                         StateChanged("active")
                                │
                         ◄──D-Bus── Extension updates UI
```

### Plugin/Extension Interface

Not applicable for v1 — no plugin system. The architecture is monolithic service + thin extension. Future extensibility could add a plugin interface for custom device tracking or traffic shaping, but this is not in scope.

## 7. Rendering / Output Layer

The extension renders UI in GNOME Shell's Quick Settings panel using GJS and GObject. Key components:

- **Toggle button:** `GObject.registerClass` custom button matching GNOME Quick Settings style.
- **Status label:** Shows SSID and device count when active.
- **Preferences window:** `Adw.PreferencesWindow` (if libadwaita is available in the extension context) or custom `St.Widget` layout.
- **QR code:** Rendered as a `Clutter.Image` from a PNG generated by the Rust service, or rendered client-side in GJS using a lightweight QR library.

## 8. Data Model

(See `docs/prd.md` section 10 for the full type definitions.)

Key types: `HotspotState`, `HotspotConfig`, `Band`, `ConnectedDevice`, `DataUsage`, `ScheduleEntry`, `RepeatRule`.

## 9. Build, Deployment & Config

### Build Steps

1. `cargo build --release` — builds `reecho-service` and `reecho-cli`.
2. `npm run build` — builds the GNOME Shell extension.
3. `flatpak-builder` — packages everything into a Flatpak.

### Hosting & Deployment

- Distributed via Flathub or a custom Flatpak repository.
- No server-side component.

### Configuration

| Config Key | Default | Location |
|------------|---------|----------|
| `ssid` | `Reecho-{hostname}` | `~/.config/reecho/config.toml` |
| `password` | (generated on first run) | `~/.config/reecho/config.toml` |
| `band` | `5GHz` | `~/.config/reecho/config.toml` |
| `data_limit` | `None` (unlimited) | `~/.config/reecho/config.toml` |
| `schedule` | `[]` (no schedule) | `~/.config/reecho/config.toml` |
| `blacklist` | `[]` (no blacklisted devices) | `~/.config/reecho/config.toml` |

## 10. Error Handling & Resilience

| Failure | Detection | Behavior |
|---------|-----------|----------|
| hostapd fails to start | Process exit code / D-Bus error | Set state to `Failed`, show error in extension, log to journal |
| NetworkManager unavailable | D-Bus connection error | Service fails to start; systemd restarts it |
| AP+STA not supported | `iw` feature check / driver error | Force mode with warning; log chipset info |
| hostapd crashes mid-session | Process exit signal | Set state to `Inactive`, notify user, attempt cleanup |
| Extension loses D-Bus connection | D-Bus signal timeout | Extension shows "Service unavailable" state |
| Data limit reached | Internal counter | Deactivate hotspot, emit `DataLimitReached` signal |
| Invalid D-Bus input | Input validation | Return error, do not modify state |

## 11. Security & Privacy

- Service runs as user-level systemd unit — no root required.
- D-Bus interface is user-session scoped — not accessible from other users.
- No outbound network calls from the service — fully offline.
- Wi-Fi credentials come from NetworkManager, not stored by Reecho.
- Config file permissions: `0600` (user-only read/write).
- No telemetry, no analytics, no phone-home.

(See `docs/rules/security.md` for the full invariant list.)

## 12. Testing Strategy

- **Unit tests:** Rust `#[cfg(test)]` modules for business logic (device tracking, scheduling, limits, config).
- **Integration tests:** D-Bus interface contract tests using mock NetworkManager/hostapd.
- **E2E tests:** Manual — Flatpak install, toggle hotspot, verify on real hardware.
- **Extension tests:** Lint only for v1; manual verification in GNOME Shell.

(See `docs/rules/testing.md` for the full testing rules.)

## 13. Performance Budget & Lazy Paths

| Metric | Budget |
|--------|--------|
| Hotspot activation latency | < 5 seconds (toggle → active) |
| Service memory (RSS) | < 20 MB |
| Extension memory (RSS) | < 5 MB |
| Device list poll interval | 2 seconds |
| Bandwidth calculation window | 5-second rolling average |

**Lazy paths:**
- Device list is only polled when hotspot is active.
- Bandwidth stats are only calculated when the device list UI is open.
- Scheduling timer only runs when at least one schedule is configured.

## 14. Open Architecture Decisions

| Decision | Status | Notes |
|----------|--------|-------|
| hostapd integration method | Open | D-Bus control interface vs. subprocess with stdin/stdout pipes |
| QR code generation location | Open | Rust service generates PNG vs. extension generates client-side |
| Flatpak D-Bus permissions | Open | Need to verify `org.freedesktop.NetworkManager` is accessible from sandbox |
| Bandwidth monitoring source | Open | `/proc/net/dev` polling vs. `conntrack` vs. `hostapd_cli` stats |

## 15. Related Documents

- `docs/prd.md` — Product requirements
- `docs/techstack.md` — Stack rationale
- `docs/implementationplan.md` — Build runbook
- `docs/rules/security.md` — Security invariants
- `docs/rules/testing.md` — Testing strategy

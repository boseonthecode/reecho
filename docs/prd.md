# Reecho — Product Requirements Document

## 1. Product Overview

**Reecho** is a Linux desktop tool that lets a machine stay connected to Wi-Fi and broadcast a hotspot simultaneously, sharing the same connection. It integrates into GNOME Quick Settings as a native-feeling toggle.

**Elevator Pitch:** "Mobile Hotspot for Linux — one toggle, your laptop becomes a hotspot."

## 2. Problem & Opportunity

Linux desktop users cannot easily share their Wi-Fi connection with other devices. Windows and Android have built-in "Mobile Hotspot" features; Linux does not. The hardware supports it (AP+STA concurrent mode), but there is no user-friendly interface. Users resort to command-line tools, carry separate travel routers, or simply go without. Reecho fills this gap with a first-class GNOME integration.

(See `docs/problemstatement.md` for the full non-technical problem statement.)

## 3. Goals & Success Metrics

| Goal | Success Metric (guiding, not contractual) |
|------|------------------------------------------|
| Make hotspot sharing effortless on Linux | User can enable hotspot in < 3 seconds from Quick Settings |
| Feel like a native GNOME feature | No visible difference from built-in toggles in Quick Settings |
| Work cross-distro | Runs on Ubuntu, Fedora, Arch, and Debian-based distros via Flatpak |
| Be reliable | Hotspot activates on first toggle > 95% of the time on supported chipsets |
| Be safe | No user data leaves the machine; no telemetry |

## 4. Non-Goals (v1)

- KDE Plasma, XFCE, or other desktop environment integration.
- Fallback to a second USB Wi-Fi dongle if AP+STA is unsupported.
- Advanced traffic shaping, QoS, or per-device speed limits.
- VPN passthrough or routing.
- Web UI or remote management.
- Support for Ethernet-to-Wi-Fi bridging (Wi-Fi sharing only).
- Root-level service or system-wide installation.

## 5. Target Users & Personas

### Persona 1: Laptop Linus (Primary)
- **Who:** Developer, 28, uses Fedora on a ThinkPad.
- **Need:** Quick way to share hotel/cafe Wi-Fi with phone while traveling.
- **Current workaround:** Tethers phone via USB, which drains battery.
- **Success:** Opens Quick Settings, taps toggle, phone connects in seconds.

### Persona 2: Power User Priya
- **Who:** Sysadmin, 35, uses Arch on a desktop with USB Wi-Fi adapter.
- **Need:** Scriptable hotspot for lab/testing environments.
- **Current workaround:** `nmcli` + `hostapd` scripts that break on reconnection.
- **Success:** `reecho on --ssid LabNet --password test123` from terminal.

### Persona 3: Switcher Sam
- **Who:** Former Windows user, 22, uses Ubuntu.
- **Need:** "Where's the mobile hotspot button?" — expects it to just work.
- **Current workaround:** Doesn't know it's possible on Linux.
- **Success:** Discovers toggle in Quick Settings, uses it without reading docs.

## 6. Key User Journeys

### Journey 1: First-Time Hotspot Activation
1. User is connected to Wi-Fi.
2. User opens GNOME Quick Settings panel.
3. User sees "Hotspot" toggle (off by default).
4. User taps toggle → hotspot activates, SSID shown, devices can connect.
5. User taps toggle again → hotspot stops, Wi-Fi connection unchanged.

### Journey 2: Configure Hotspot Settings
1. User long-presses or clicks the Hotspot toggle (or opens preferences).
2. Settings panel shows: SSID field, password field, band selector (2.4/5 GHz).
3. User changes SSID and password, saves.
4. Next hotspot activation uses new settings.

### Journey 3: View Connected Devices
1. Hotspot is active.
2. User opens device list from Quick Settings or preferences.
3. Shows connected devices with names, MAC addresses, and bandwidth usage.
4. User can blacklist a device → it is disconnected and blocked.

### Journey 4: Schedule Hotspot
1. User opens scheduling settings.
2. User sets "On at 9:00 AM, off at 5:00 PM, weekdays."
3. Hotspot activates and deactivates automatically on schedule.

### Journey 5: Data Cap Enforcement
1. User sets a 1 GB data limit.
2. Hotspot tracks cumulative data usage.
3. When limit is reached, hotspot deactivates automatically.
4. User is notified (notification or status indicator).

## 7. Functional Requirements

### FR-1: Hotspot Control

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-1.1 | Toggle hotspot on/off from GNOME Quick Settings | P0 |
| FR-1.2 | Hotspot shares the current Wi-Fi connection automatically | P0 |
| FR-1.3 | Wi-Fi connection remains active when hotspot is on | P0 |
| FR-1.4 | Hotspot deactivation does not disconnect from Wi-Fi | P0 |
| FR-1.5 | Status indicator shows hotspot state (on/off/activating) | P0 |
| FR-1.6 | Force AP+STA mode even on unsupported chipsets, with visible warning | P0 |

### FR-2: Configuration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-2.1 | User can set custom SSID (max 32 bytes) | P0 |
| FR-2.2 | User can set password (min 8, max 63 bytes, WPA2/WPA3) | P0 |
| FR-2.3 | User can select band: 2.4 GHz or 5 GHz | P1 |
| FR-2.4 | Settings persist across restarts | P0 |
| FR-2.5 | Default SSID: "Reecho-{hostname}" | P1 |

### FR-3: Device Management

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-3.1 | Show list of connected devices (name, MAC, IP) | P1 |
| FR-3.2 | Show per-device bandwidth usage (session total + current rate) | P1 |
| FR-3.3 | Show total hotspot data usage | P1 |
| FR-3.4 | Blacklist a device by MAC address | P1 |
| FR-3.5 | Blacklisted device is disconnected immediately | P1 |

### FR-4: QR Code

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-4.1 | Generate QR code for hotspot credentials (SSID + password) | P1 |
| FR-4.2 | QR code updates when settings change | P1 |
| FR-4.3 | QR code is scannable by Android/iOS camera apps | P0 |

### FR-5: Scheduling

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-5.1 | User can set auto-on time | P2 |
| FR-5.2 | User can set auto-off time | P2 |
| FR-5.3 | User can set repeat schedule (weekdays, daily, once) | P2 |
| FR-5.4 | Scheduled activation respects current Wi-Fi connection state | P2 |

### FR-6: Data Limits

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-6.1 | User can set a total data cap (e.g., 1 GB, 5 GB) | P2 |
| FR-6.2 | Hotspot deactivates when cap is reached | P2 |
| FR-6.3 | User is notified when cap is approaching (80%) and reached (100%) | P2 |
| FR-6.4 | Data usage counter resets on user action or schedule | P2 |

### FR-7: CLI

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-7.1 | `reecho on [--ssid X] [--password Y] [--band Z]` starts hotspot | P1 |
| FR-7.2 | `reecho off` stops hotspot | P1 |
| FR-7.3 | `reecho status` shows current state, connected devices, usage | P1 |
| FR-7.4 | `reecho config set/get` manages configuration | P2 |
| FR-7.5 | `reecho devices list` shows connected devices | P2 |
| FR-7.6 | `reecho blacklist add/remove` manages device blacklist | P2 |

### FR-8: Extension Integration

| ID | Requirement | Priority |
|----|-------------|----------|
| FR-8.1 | Extension appears in GNOME Quick Settings panel | P0 |
| FR-8.2 | Toggle state syncs with service state in real time | P0 |
| FR-8.3 | Extension shows hotspot SSID when active | P1 |
| FR-8.4 | Extension preferences window for settings | P1 |
| FR-8.5 | Extension gracefully handles service unavailable | P0 |

## 8. Non-Functional Requirements

| Category | Requirement | Budget/Target |
|----------|-------------|---------------|
| Privacy | No data leaves the machine; no telemetry, no analytics | Hard invariant |
| Performance | Hotspot activation latency (toggle → active) | < 5 seconds on supported hardware |
| Performance | Service memory footprint | < 20 MB RSS |
| Performance | Extension memory footprint | < 5 MB RSS |
| Compatibility | Supported distros | Ubuntu 22.04+, Fedora 38+, Arch (latest), Debian 12+ |
| Compatibility | GNOME version | 42+ (Quick Settings introduced in 42) |
| Compatibility | Wi-Fi chipsets | Best-effort; force mode for unsupported |
| Responsiveness | UI toggle response time | < 500ms to visual feedback |
| Accessibility | Keyboard navigation | Full keyboard access to all extension controls |
| Accessibility | Screen reader | ARIA labels on all interactive elements |
| Theming | Light/dark mode | Respect system theme; no hardcoded colors |
| Security | D-Bus interface authorization | User-level only; no system-wide access |
| Reliability | Service auto-restart on crash | systemd user unit with `Restart=on-failure` |
| Reliability | Graceful degradation | If hostapd fails, show error, don't crash |

## 9. Technical Architecture & Feasibility

### Platform Decision

Linux desktop (GNOME only for v1), distributed as Flatpak.

**Rationale:** GNOME Quick Settings is the most widely used GNOME Shell panel for system toggles. Flatpak provides cross-distro compatibility without maintaining multiple packaging formats.

### Stack

- **Service:** Rust, systemd user unit, D-Bus API (`zbus` crate).
- **Extension:** GJS (GNOME Shell extension), Quick Settings integration.
- **CLI:** Rust, same D-Bus interface as extension.
- **Networking:** NetworkManager (D-Bus API) + hostapd (process management).
- **QR Codes:** Rust crate (`qrcode` or similar).

### Processing

All processing happens locally on the user's machine. No server, no cloud, no external network calls.

### Architecture Sketch

```
┌─────────────────────────────────────────────────┐
│                  GNOME Shell                     │
│  ┌───────────────────────────────────────────┐  │
│  │        Quick Settings Panel                │  │
│  │  ┌─────────────────────────────────────┐  │  │
│  │  │   Reecho Toggle  [ON/OFF]           │  │  │
│  │  │   Status: "MyHotspot" • 2 devices   │  │  │
│  │  └─────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────┘  │
│                    │ D-Bus                       │
│                    ▼                             │
│  ┌───────────────────────────────────────────┐  │
│  │         reecho-service (Rust)             │  │
│  │  ┌──────────┐ ┌──────────┐ ┌──────────┐  │  │
│  │  │ Network  │ │ hostapd  │ │ Device   │  │  │
│  │  │ Manager  │ │ Manager  │ │ Tracker  │  │  │
│  │  └──────────┘ └──────────┘ └──────────┘  │  │
│  └───────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
         │                          │
         ▼                          ▼
┌──────────────┐          ┌──────────────────┐
│ NetworkManager│          │     hostapd      │
│  (D-Bus API)  │          │  (process/D-Bus) │
└──────────────┘          └──────────────────┘
```

## 10. Data & State Model

```rust
// Hotspot state
enum HotspotState {
    Inactive,
    Activating,
    Active,
    Failed { reason: String },
}

// Hotspot configuration
struct HotspotConfig {
    ssid: String,          // max 32 bytes
    password: String,      // min 8, max 63 bytes
    band: Band,            // 2.4GHz or 5GHz
    enabled: bool,
}

enum Band {
    Band2_4Ghz,
    Band5Ghz,
}

// Connected device
struct ConnectedDevice {
    mac: MacAddress,
    ip: IpAddr,
    name: Option<String>,  // hostname from DHCP/ARP
    connected_at: DateTime<Utc>,
    bytes_rx: u64,
    bytes_tx: u64,
    rate_rx: f64,          // bytes/sec (current)
    rate_tx: f64,
}

// Data usage tracking
struct DataUsage {
    total_rx: u64,
    total_tx: u64,
    limit: Option<u64>,    // bytes, None = unlimited
    started_at: DateTime<Utc>,
}

// Schedule entry
struct ScheduleEntry {
    on_time: NaiveTime,
    off_time: NaiveTime,
    repeat: RepeatRule,
    enabled: bool,
}

enum RepeatRule {
    Once(NaiveDate),
    Daily,
    Weekdays,
    Custom(Vec<Weekday>),
}
```

## 11. UI/UX Requirements

The UI is a GNOME Shell extension integrated into Quick Settings. It follows the native GNOME look and feel — no custom design system. See `docs/visualdesign.md` is skipped for this project; the extension uses GNOME's built-in styling and the Quick Settings panel conventions.

Key UI elements:
- **Toggle button** in Quick Settings panel (matches Wi-Fi/Bluetooth toggle style).
- **Status text** below toggle: hotspot SSID and device count when active.
- **Preferences window** (accessible via long-press or gear icon): SSID, password, band, scheduling, data limits, blacklist, QR code display.
- **Notification** on data cap approaching/reached.

## 12. Roadmap & Milestones

### M0: Spike (Feasibility)
- Verify AP+STA concurrent mode works on target hardware.
- Prototype Rust service with D-Bus interface.
- Prototype GNOME Shell extension with basic toggle.
- **Exit criteria:** Toggle activates hotspot on at least one chipset; extension communicates with service.

### M1: MVP
- Hotspot on/off from Quick Settings.
- SSID and password configuration.
- Wi-Fi connection maintained during hotspot.
- Basic device list.
- CLI: `reecho on/off/status`.
- Flatpak packaging.
- **Exit criteria:** A user can install the Flatpak, toggle hotspot, connect a phone, and see it in the device list.

### M2: Refinement
- Band selection.
- QR code generation.
- Device blacklisting.
- Bandwidth usage display.
- Scheduling.
- Data limits.
- Error handling and unsupported chipset warnings.

### M3: Roadmap (Future)
- KDE Plasma integration.
- Second dongle fallback.
- VPN passthrough.
- Web UI for remote management.

## 13. Assumptions, Risks & Open Questions

### Assumptions

1. Target users have a Wi-Fi chipset that supports AP+STA concurrent mode (or are willing to use force mode).
2. NetworkManager is installed and running (standard on all target distros).
3. GNOME 42+ is the target desktop environment.
4. The user has `hostapd` available (may need to be installed as a Flatpak permission or dependency).

### Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| AP+STA not supported on many chipsets | High — core feature doesn't work | Force mode with clear warning; document supported chipsets; prioritize chipset compatibility research |
| hostapd + NetworkManager conflicts | High — network instability | Careful lifecycle management in the Rust service; test on multiple distros |
| Flatpak sandboxing blocks D-Bus access to NM/hostapd | High — service can't function | Request appropriate Flatpak permissions; document manual install as fallback |
| GNOME Shell extension crashes affect desktop | Medium — poor UX | Keep extension thin; all logic in service; crash isolation via systemd |
| D-Bus interface design changes break CLI/extension sync | Medium — maintenance burden | Versioned D-Bus interface; shared interface definition in `shared/` |

### Open Questions

1. Exact hostapd integration approach (D-Bus via `hostapd` control interface vs. subprocess management).
2. How to detect chipset AP+STA capability reliably (iw features, driver-specific queries).
3. Flatpak permission model for accessing NetworkManager and hostapd system services.
4. Whether the service needs to run as root for hostapd management, or if user-level is sufficient.

### Decisions (Recorded)

| Decision | Choice | Date |
|----------|--------|------|
| Backend language | Rust | 2026-08-23 |
| UI framework | GNOME Shell extension (GJS) | 2026-08-23 |
| Architecture | Separate service + D-Bus | 2026-08-23 |
| CLI | Yes, Rust, same D-Bus interface | 2026-08-23 |
| License | MIT | 2026-08-23 |
| Distribution | Flatpak | 2026-08-23 |

## 14. Glossary

| Term | Definition |
|------|------------|
| **AP+STA** | Access Point + Station — a Wi-Fi chipset operating as both a hotspot (AP) and a client (STA) simultaneously |
| **NetworkManager** | The standard Linux network management daemon; manages Wi-Fi, Ethernet, VPN connections |
| **hostapd** | User-space daemon for creating Wi-Fi access points (hotspots) |
| **D-Bus** | Linux inter-process communication mechanism; used for service ↔ extension communication |
| **Quick Settings** | The GNOME Shell panel with system toggles (Wi-Fi, Bluetooth, etc.), introduced in GNOME 42 |
| **Flatpak** | Cross-distribution Linux packaging format with sandboxing |
| **GJS** | GNOME JavaScript — the JavaScript runtime used by GNOME Shell extensions |
| **SSID** | Service Set Identifier — the name of a Wi-Fi network |
| **MAC address** | Media Access Control address — unique hardware identifier for a network interface |

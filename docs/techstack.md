# Reecho — Tech Stack

## 1. Summary

| Layer | Choice |
|-------|--------|
| Backend service | Rust |
| CLI | Rust |
| Desktop UI | GNOME Shell extension (GJS) |
| IPC | D-Bus (user session bus) |
| Wi-Fi management | NetworkManager (D-Bus API) + hostapd (process/D-Bus) |
| QR codes | Rust crate (`qrcode` or `qr2term`) |
| Systemd integration | `systemd-user-units` (manual or Flatpak hook) |
| Packaging | Flatpak |
| License | MIT |

## 2. Language & Framework

### Rust (service + CLI)

**Why Rust:**
- Memory safety without garbage collection — critical for a long-running service.
- Excellent D-Bus support via `zbus` (pure Rust, async, well-maintained).
- Strong ecosystem for process management (`tokio`, `nix`).
- `systemd` integration via `zbus` or `libsystemd` bindings.
- Cross-compilation support for Flatpak.
- Compiles to a single static binary — no runtime dependencies.

**Crate choices:**
- `zbus` — D-Bus client/server (async, pure Rust).
- `tokio` — Async runtime.
- `thiserror` — Typed error definitions.
- `serde` / `toml` — Configuration serialization.
- `qrcode` or `qr2term` — QR code generation (TBD at implementation time).
- `chrono` — Time/scheduling.
- `tracing` — Structured logging (journal-compatible).

### GJS (GNOME Shell extension)

**Why GJS:**
- Standard for GNOME Shell extensions.
- Direct access to GNOME Shell APIs (Quick Settings, `GObject`, `St`, `Clutter`).
- No additional runtime needed — ships with GNOME.

**Caveats:**
- GJS uses its own import system (`imports.gi.*`), not ES modules.
- Extension runs in the GNOME Shell compositor process — crashes affect the desktop.
- Keep extension code thin; delegate to the Rust service.

## 3. Styling

Not a major concern for v1 — the extension uses GNOME Shell's built-in styling and the Quick Settings panel conventions.

- **CSS:** Extension stylesheet uses CSS custom properties where possible.
- **Colors:** Reference `theme_node` colors, not hardcoded hex values.
- **Fonts:** System fonts (GNOME default).
- **Icons:** GNOME icon theme; hotspot icon from standard icon set or custom SVG.

## 4. Core Processing / Domain Libraries

| Library | Purpose |
|---------|---------|
| `zbus` | D-Bus interface for NetworkManager, hostapd, and internal service IPC |
| `hostapd` (system binary) | AP mode — creating and managing the Wi-Fi access point |
| `NetworkManager` (system D-Bus) | Wi-Fi connection management, AP+STA configuration |
| `iw` (system binary) | Wi-Fi capability detection (AP+STA support check) |
| `arp` / `ip neigh` | Device discovery (fallback if hostapd client list unavailable) |

## 5. State & Persistence

- **In-memory:** Hotspot state, connected devices, bandwidth counters, schedule timers.
- **Filesystem:** `~/.config/reecho/config.toml` for user configuration.
- **Ephemeral:** Bandwidth stats not persisted — recomputed on service start.
- **Systemd:** Service state managed by systemd (start, stop, restart, status).

## 6. Hosting & Deployment

- **Distribution:** Flatpak via Flathub or custom repository.
- **Runtime:** No server — fully local, peer-to-peer hotspot sharing.
- **Updates:** Flatpak update mechanism.
- **Service management:** systemd user unit, auto-started on login (via GNOME or systemd user slice).

### Flatpak Permissions

| Permission | Why |
|------------|-----|
| `org.freedesktop.NetworkManager` | D-Bus access to NetworkManager |
| `org.freedesktop.systemd1` | User-level systemd unit management |
| `org.freedesktop.hostapd` (if available) | D-Bus access to hostapd |
| Filesystem: `~/.config/reecho` | Config persistence |
| Filesystem: `/tmp` (limited) | Temporary hostapd config files |

## 7. Quality Tooling

| Tool | Purpose | Command |
|------|---------|---------|
| `cargo clippy` | Rust linting | `cargo clippy -- -D warnings` |
| `cargo fmt` | Rust formatting | `cargo fmt --check` |
| `cargo test` | Rust unit + integration tests | `cargo test` |
| `cargo audit` | Dependency vulnerability scanning | `cargo audit` |
| `npm run lint` | Extension JS linting | `npm run lint` |
| `flatpak-builder` | Flatpak packaging | `flatpak-builder builddir com.reecho.Reecho.yml` |

**Fixtures / sample assets:**
- Test fixtures in `tests/fixtures/` (sample D-Bus responses, config files).
- Generated via scripts if needed, but committed to the repo.

## 8. Performance & Compatibility Budget

| Metric | Budget |
|--------|--------|
| Hotspot activation latency | < 5 seconds |
| Service memory (RSS) | < 20 MB |
| Extension memory (RSS) | < 5 MB |
| Device list poll interval | 2 seconds |
| Bandwidth calculation window | 5-second rolling average |
| Supported GNOME | 42+ |
| Supported distros | Ubuntu 22.04+, Fedora 38+, Arch, Debian 12+ |
| Supported architectures | x86_64, aarch64 |

## 9. Explicitly Not Used (v1)

| Technology | Why Rejected |
|------------|-------------|
| Python | Slower startup, larger memory footprint, weaker D-Bus ecosystem compared to Rust/zbus |
| C | Manual memory management, higher risk of bugs, no benefit over Rust for this use case |
| Electron / web UI | Heavy runtime, not native GNOME feel, contradicts "first-class system feature" goal |
| Second USB dongle fallback | Complexity for v1; force mode with warning is the deliberate trade-off |
| SQLite | Config is small and simple; TOML file is sufficient |
| Prometheus / metrics | No telemetry; stats are ephemeral and local |
| Docker / containerization | Flatpak is the Linux desktop standard; Docker adds unnecessary complexity |

## 10. Related Documents

- `docs/architecture.md` — How the stack is assembled into a system
- `docs/prd.md` — What the stack must deliver
- `docs/implementationplan.md` — How to build it, commit by commit

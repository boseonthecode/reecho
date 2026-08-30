# Reecho — Idea

> A raw, working dump of the idea. This file is the source of truth. It is
> expected to be messy; the docs in `docs/` are the refined version.

## One-liner

Simultaneous Wi-Fi + Hotspot for Linux, integrated into the desktop control panel.

## Problem

On most Linux distros, if your machine is connected to a Wi-Fi network, you can't
also turn that same Wi-Fi adapter into a hotspot to share the connection. This is
possible on Windows (Mobile Hotspot) and Android, but on Linux it usually requires
manually invoking `nmcli`/`hostapd`, and even then it's not integrated anywhere in
the system UI. There's no simple toggle to say "share the network I'm currently on."

## Users

- Linux laptop users who want to share their Wi-Fi connection with other devices
  (phones, tablets, other laptops) without carrying a separate hotspot device.
- Developers/power users who need a quick, scriptable way to set up a shared network.
- Anyone frustrated by the lack of a native "Mobile Hotspot" equivalent on Linux.

## What it does (key capabilities)

- Lets a Linux machine stay connected to Wi-Fi *and* broadcast a hotspot simultaneously,
  sharing that same connection (AP+STA concurrent mode).
- Integrates into GNOME Quick Settings as a native-feeling toggle.
- On/off toggle to enable/disable the hotspot.
- Automatically shares whatever Wi-Fi network the machine is currently connected to.
- User-configurable SSID, password, and band selection (2.4GHz / 5GHz).
- List of connected devices with basic bandwidth usage per device.
- QR code generation for easy phone/device connection.
- Scheduling — set times for the hotspot to auto turn on/off.
- Data limits — cap total data shared through the hotspot.
- Device blacklisting — block specific devices from connecting.

## What it explicitly does NOT do (for now)

- No KDE Plasma or other DE integration (GNOME only for v1).
- No fallback to a second USB Wi-Fi dongle — if AP+STA isn't supported, force it
  with a warning.
- No advanced traffic shaping or QoS beyond basic data caps.
- No VPN passthrough or routing.

## Constraints & invariants

- Must work on Linux with NetworkManager (primary interface).
- Packaged as a Flatpak for cross-distro compatibility.
- Background service handles AP+STA orchestration; GNOME Shell extension provides UI.
- Privacy: no user data leaves the machine; all processing is local.
- No server component — fully offline, peer-to-peer hotspot sharing.

## Stack & platform (if decided)

- Platform: Linux desktop (GNOME only for v1).
- Distribution: Flatpak.
- Backend: Background service in **Rust** (D-Bus interface to NetworkManager + hostapd).
- UI: GNOME Shell extension (GJS) in Quick Settings, talks to the service over D-Bus.
- CLI: Rust CLI tool, same D-Bus interface as the extension.
- NetworkManager (`nmcli` / D-Bus API) + `hostapd` for AP+STA.
- Licensing: **MIT**.
- QR code: Rust crate (e.g., `qrcode` or `qr2term`).
- Architecture: service runs as a systemd user unit; extension is installed separately.
  Separate service + D-Bus chosen over embedding logic in the extension (standard GNOME
  pattern, testable independently, CLI works without extension, service crashes don't
  affect GNOME Shell).

## Design / aesthetic reference (if UI product)

- Native GNOME Quick Settings look — should feel like a built-in system toggle,
  not a third-party app.

## Open questions

- Language choice for the background service (Python vs. Rust vs. other).
- Exact D-Bus interface design between the extension and the service.
- How to detect and handle AP+STA "force" mode across different Wi-Fi drivers/chipsets.
- UX for unsupported chipset warning — how prominent, can advanced users retry?
- Whether to expose a CLI/API in addition to the GUI for scripting/automation.
- Systemd service vs. user-level daemon management.
- Licensing (GPL-3.0? MIT?).
- Testing strategy — how to test Wi-Fi concurrent mode without real hardware.
- Whether QR code generation happens client-side (pure JS/WASM) or via a library.
- Data model for device tracking and bandwidth monitoring.

## Notes / context

- This is inspired by Android's "Mobile Hotspot" and Windows' "Mobile Hotspot"
  features — simple, first-class, no third-party tools needed.
- AP+STA concurrent mode is supported by many modern Wi-Fi chipsets but not all;
  the "force" mode with warning is a deliberate trade-off for v1.
- Background service + GNOME Shell extension split is the standard pattern for
  system-level GNOME integrations.

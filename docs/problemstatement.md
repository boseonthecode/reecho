# Reecho — Problem Statement

## Core Problem

Linux desktop users cannot easily share their Wi-Fi internet connection with other devices. On Windows and Android, this is a built-in feature called "Mobile Hotspot" — a single toggle that turns your laptop into a wireless access point while it stays connected to Wi-Fi. On Linux, this capability exists at the driver level (AP+STA concurrent mode), but there is no user-friendly way to activate or manage it. Users must manually run command-line tools like `nmcli` or `hostapd`, understand wireless networking concepts, and accept that the process is fragile and unintegrated.

## Who Is Affected

- **Laptop users on the go** who want to give their phone or tablet internet access from their laptop's Wi-Fi connection, without carrying a separate travel router.
- **Developers and power users** who need a quick way to create a shared network for testing, prototyping, or on-site work — but currently resort to scripts or manual `nmcli` invocations.
- **Anyone switching from Windows or Android** who expects a "share my connection" toggle in their desktop environment and finds it missing.

## What Happens Today Without It

Users either carry a separate hotspot device, tether via USB (which drains the phone battery), or struggle through manual command-line setup that breaks when they disconnect and reconnect to a different network. The lack of a native integration means most Linux users simply don't use this feature, even though the hardware supports it.

## Desired State

A single toggle in the GNOME Quick Settings panel that, when enabled, immediately shares the machine's current Wi-Fi connection as a hotspot — no command line, no third-party tools, no configuration files. The experience should feel like it belongs in the operating system, not like an add-on.

# Reecho — Security & Privacy Rules

## Principles

- **Privacy by construction:** No user data leaves the machine. All processing is local. No telemetry, no analytics, no external network calls from the service.
- **Sealed/static interfaces:** The D-Bus interface exposes only the operations defined in the spec — no arbitrary command execution, no dynamic method dispatch.
- **No secrets in code:** No hardcoded credentials, API keys, or passwords. Wi-Fi credentials come from NetworkManager, not from Reecho's config.
- **Small attack surface:** The service runs with minimal privileges (user-level systemd unit, not root). The extension is a thin UI layer with no direct system access.

## Data Flow Rules

| Path | Allowed | Forbidden |
|------|---------|-----------|
| Extension → Service (D-Bus) | Control commands (toggle, config, status) | Raw file access, arbitrary shell commands |
| Service → NetworkManager (D-Bus) | Wi-Fi connect/disconnect, AP create/destroy | Modifying system-wide NM config |
| Service → hostapd (process/D-Bus) | Start/stop, config, client list | Accessing hostapd's internal state beyond API |
| Service → filesystem | Config file in `~/.config/reecho/` | Writing to system directories, /tmp with predictable names |
| Service → network | None (fully offline) | Any outbound HTTP, DNS, or socket connections |

## Storage

- Config stored in `~/.config/reecho/config.toml` (user-writable, not world-readable).
- No user files, raw bytes, or network credentials stored beyond what NetworkManager already holds.
- Bandwidth stats are ephemeral (in-memory or volatile cache) — not persisted to disk.

## Dependencies

- Lockfile (`Cargo.lock`) committed and intentional.
- Run `cargo audit` in CI — no known vulnerabilities in dependencies.
- No WASM or compiled binary dependencies without explicit review.
- Pin versions for critical dependencies (hostapd interaction, D-Bus crate).

## Input Validation & Rendering

- All D-Bus method inputs validated against the interface spec before processing.
- SSID and password inputs sanitized — no control characters, length limits enforced (SSID: 32 bytes max, password: 63 bytes max per 802.11 spec).
- User-provided strings (device names, SSID) never rendered as HTML — GJS uses `Pango` for text, which is safe.
- QR code data validated before rendering — no injection via crafted SSID/password.

## Worker / Memory

- Service runs in its own systemd user unit, isolated from GNOME Shell process.
- No shared memory between extension and service — D-Bus only.
- Large data (device lists, bandwidth stats) paginated or streamed, not loaded entirely into memory.

## Secrets & Environment

- No `.env` files or environment variables containing credentials.
- Wi-Fi passwords read from NetworkManager secrets, not stored by Reecho.
- Git hygiene: `.gitignore` excludes `*.pem`, `*.key`, `.env`, target directories.

## Code Review Security Checklist

Before any merge:
- [ ] No `unwrap()` in production paths
- [ ] No hardcoded credentials or secrets
- [ ] D-Bus inputs validated
- [ ] File operations use safe paths (no symlink following, no world-writable dirs)
- [ ] No `unsafe` blocks without `// SAFETY:` justification
- [ ] Error messages don't leak sensitive info (no passwords, no raw MAC addresses in logs)

## Vulnerability Reporting

- Report security issues privately via GitHub security advisories.
- Do not open public issues for security vulnerabilities.

# Reecho — Root Instructions

## Project Snapshot

Reecho is a Linux desktop tool that lets a machine stay connected to Wi-Fi and broadcast a hotspot simultaneously, sharing the same connection. It integrates into GNOME Quick Settings as a first-class toggle. The backend is a Rust service exposed over D-Bus; the UI is a GNOME Shell extension (GJS); a Rust CLI provides scripting access. Distributed as a Flatpak. Licensed MIT.

Key facts:
- **Platform:** Linux (GNOME only for v1)
- **Backend:** Rust service, systemd user unit, D-Bus API
- **UI:** GNOME Shell extension (GJS), Quick Settings integration
- **CLI:** Rust, same D-Bus interface
- **Networking:** NetworkManager + hostapd for AP+STA concurrent mode
- **Distribution:** Flatpak
- **License:** MIT

## Read These Docs First

| File | Purpose |
|------|---------|
| `docs/progress.md` | Session continuity log — read first, update last |
| `docs/problemstatement.md` | The problem Reecho solves, written for a non-technical reader |
| `docs/prd.md` | Full product requirements with prioritized feature tables |
| `docs/architecture.md` | System architecture, component design, data flow, security |
| `docs/techstack.md` | Chosen stack, rationale, and rejected alternatives |
| `docs/implementationplan.md` | Commit-by-commit build runbook |
| `docs/rules/codestyle.md` | Code style conventions for Rust, GJS, and CSS |
| `docs/rules/testing.md` | Testing rules and strategy |
| `docs/rules/security.md` | Security and privacy invariants |

## Commands

| Command | Purpose |
|---------|---------|
| `cargo build` | Build the Rust service and CLI |
| `cargo test` | Run unit and integration tests |
| `cargo clippy` | Lint Rust code |
| `cargo fmt --check` | Check formatting |
| `npm run lint` | Lint GNOME Shell extension (GJS) |
| `npm run build` | Build extension for distribution |
| `flatpak-builder` | Build Flatpak package |

**Gate:** lint + typecheck + test must pass before any task is declared done.

## Codebase Map

```
src/
  service/          # Rust background service (D-Bus server, NetworkManager/hostapd orchestration)
    main.rs
    dbus.rs         # D-Bus interface definition and handlers
    network.rs      # NetworkManager interaction (AP+STA setup, connection state)
    ap.rs           # hostapd management, AP lifecycle
    devices.rs      # Connected device tracking, bandwidth monitoring
    scheduler.rs    # Hotspot scheduling (auto on/off times)
    limits.rs       # Data caps and enforcement
    blacklist.rs    # Device blacklisting
    config.rs       # Configuration persistence
    error.rs        # Typed error codes
  cli/              # Rust CLI tool (same D-Bus interface as extension)
    main.rs
    commands.rs
  extension/        # GNOME Shell extension (GJS)
    extension.js    # Entry point, Quick Settings integration
    ui.js           # Panel UI components
    dbusClient.js   # D-Bus client to talk to the Rust service
    qr.js           # QR code rendering in the panel
    prefs.js        # Preferences/settings window
    stylesheet.css  # Extension styles
  shared/           # Shared types and constants (D-Bus interface definitions)
    dbus-interface.xml
    constants.rs
    constants.js
docs/               # Documentation (this tree)
AGENTS.md           # Pointer to docs/AGENTS.md
```

Hard rules:
- The `service/` and `cli/` layers must not import any GNOME/extension code.
- The `extension/` layer must not contain business logic — it delegates to the service via D-Bus.
- `shared/` contains only data definitions, no behavior.

## Workflow

1. Read `docs/progress.md` to understand current state.
2. Read the relevant design docs (`prd.md`, `architecture.md`, `techstack.md`).
3. Read the rules (`docs/rules/*.md`) before writing code.
4. Implement the change.
5. Run `cargo clippy`, `cargo fmt --check`, `cargo test` (for Rust), and `npm run lint` (for extension).
6. All checks must pass before declaring the task done.
7. Update `docs/progress.md` with what was done, what changed, and what's next.

## Progress Log

`docs/progress.md` is the single source of truth for project state. The next session must be able to continue without asking anything already decided. Read first, update last, over-record rather than strand.

## Commit Rules

- Logical batches only — group related changes into one commit.
- Keep the whole project under ~30 commits.
- Conventional Commits style (`feat:`, `fix:`, `docs:`, `chore:`, `refactor:`, `test:`).
- Never commit unless explicitly asked.

## Never-Do List

- Never commit secrets, API keys, tokens, or credentials.
- Never expose or log user data, network credentials, or device identifiers.
- Never loosen strict typing (`unsafe` Rust requires explicit justification).
- Never add unvetted dependencies without discussion.
- Never ignore failing lint/typecheck/test.
- Never move privileged network operations to the main GNOME Shell thread.
- Never create or edit docs unless asked (except `docs/progress.md`).
- Never use `unwrap()` in production code paths — use proper error handling.
- Never hardcode SSID/password defaults.

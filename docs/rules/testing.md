# Reecho — Testing Rules

## Test Scripts

| Command | What it covers |
|---------|---------------|
| `cargo test` | Rust unit + integration tests |
| `cargo clippy` | Lint (must pass) |
| `cargo fmt --check` | Format (must pass) |
| `npm run lint` | Extension JS lint (must pass) |

**CI gate:** All of the above must pass before any commit.

## Unit Testing

- **Framework:** Rust's built-in `#[cfg(test)]` with `cargo test`.
- **Scope:** Test individual functions and modules in isolation.
- **Location:** Co-located test modules (`#[cfg(test)] mod tests { ... }` at the bottom of each file) for small tests; separate `tests/` directory for integration tests.
- **Coverage target:** All business logic in `service/` and `cli/` must have unit tests.

## Integration Testing

- **Framework:** Rust integration tests in `tests/` directory.
- **Scope:** Test D-Bus interface contracts, NetworkManager mock interactions, hostapd lifecycle.
- **Mocking:** Mock NetworkManager and hostapd at the D-Bus boundary using `zbus` test utilities or a mock D-Bus server. Do not mock internal modules.

## Determinism Rules

- No `sleep()` or real delays in tests — use `tokio::time::pause()` or fake timers.
- No network calls in tests — mock all D-Bus and external process interactions.
- No filesystem state依赖 — use `tempdir` for any file operations.
- No time-dependent assertions that could flake — use fixed timestamps.

## Fixtures

- Commit test fixtures to `tests/fixtures/`.
- Keep fixtures small — minimal config files, sample D-Bus responses.
- Generate fixtures via scripts if needed, but commit the generated output.

## Mocking Policy

- Mock only external boundaries: NetworkManager D-Bus, hostapd process, filesystem.
- Do not mock internal Rust types — test real behavior within the crate.
- Use trait objects or feature flags for test doubles where needed.

## E2E Testing

- **Framework:** Manual for v1 (Flatpak + real Wi-Fi hardware).
- **Critical flows to verify:**
  1. Service starts, D-Bus interface appears.
  2. Extension loads, toggle visible in Quick Settings.
  3. Toggle on → hotspot active, devices can connect.
  4. Toggle off → hotspot stopped, Wi-Fi connection maintained.
  5. CLI `reecho status` reflects current state.
  6. Scheduling: hotspot activates at configured time.
  7. Data limit: hotspot disables when cap reached.
  8. Blacklist: blocked device cannot connect.

## Rules of Thumb

- Every behavior change ships a test.
- Every bug fix ships a regression test.
- Test observable behavior, not implementation details.
- If a test is hard to write, the code may need refactoring.

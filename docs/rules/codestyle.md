# Reecho — Code Style Rules

## Rust (service + CLI)

### Strictness
- `rustfmt` with default config (project-level `rustfmt.toml` if overrides needed).
- `clippy` with all lints enabled — no `allow` annotations without justification.
- No `unsafe` blocks unless absolutely necessary and documented with a `// SAFETY:` comment.
- No `unwrap()` in production code paths — use `?` or `.expect()` with clear messages in tests only.
- No `anyhow` — use typed errors via `thiserror` for domain errors.

### Naming & Casing
- Items: `snake_case` for functions/variables/modules, `PascalCase` for types/traits/enums.
- Constants: `SCREAMING_SNAKE_CASE`.
- Files: `snake_case.rs`, one major type per file preferred.
- Modules: `snake_case`, flat where possible, nested only for logical grouping.

### Imports
- Group imports: std → external crates → local crate, separated by blank lines.
- Use explicit imports, no glob (`use foo::*`) except in test modules.
- Prefer `use crate::` for internal imports.

### Error Handling
- Define a crate-level error enum in `error.rs` using `thiserror`.
- Propagate errors with `?` — no `.unwrap()` or `.expect()` in non-test code.
- Log errors at the point of detection, not just at the boundary.

### Comments & Documentation
- No inline comments for obvious code — the code should be self-documenting.
- Doc comments (`///`) on all public items (functions, structs, enums, traits).
- Module-level doc comments (`//!`) at the top of each module file.
- `// TODO:` and `// FIXME:` are acceptable; `// HACK:` requires a tracking issue.

### Formatting
- Max line length: 100 characters (enforced by `rustfmt`).
- One blank line between functions/impls, no trailing whitespace.
- Max file length: ~500 lines; split into modules if longer.

## GNOME Shell Extension (GJS)

### Naming & Casing
- JavaScript: `snake_case` for variables/functions (GJS convention), `PascalCase` for classes.
- Files: `camelCase.js` or `snake_case.js` — stay consistent within the extension.

### Style
- Use `const`/`let`, never `var`.
- No ES modules (GJS uses its own import system: `const { GObject } = imports.gi`).
- Prefer `GObject.registerClass` for GObject-derived classes.
- Keep extension code thin — all business logic delegates to the Rust service via D-Bus.

### Comments
- JSDoc on public functions and GObject class definitions.
- No inline comments for obvious logic.

## CSS (extension stylesheet)

- Use CSS custom properties for colors (GNOME theming integration).
- No hardcoded color values — reference `theme_node` colors where possible.
- Keep selectors shallow — avoid nesting deeper than 2 levels.

## General

- All code must pass `rustfmt`, `clippy`, and `npm run lint` before commit.
- No trailing whitespace, no mixed tabs/spaces.
- UTF-8 encoding for all files.

//! Shared types and D-Bus interface definitions for Reecho.
//!
//! This crate contains data definitions only — no business logic.
//! Used by the service, CLI, and GNOME Shell extension.

pub mod constants;
pub mod types;

pub use constants::*;
pub use types::*;

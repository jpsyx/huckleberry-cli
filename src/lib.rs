//! huckleberry-cli: the library half of the command-line tool.
//!
//! The binary (`src/main.rs`) parses the arguments and hands them here, so
//! every decision this tool makes is reachable from a test without spawning a
//! process. Modules follow the pure/impure split described in
//! `docs/architecture.md`: `cli`, `config` and `theme` are data, parsing and
//! formatting; `commands` and the terminal half of `prompt` are the thin
//! shells that touch the outside world.

pub mod cli;
pub mod commands;
pub mod config;
pub mod prompt;
pub mod theme;

/// The product name shown to people, in help text and in messages. The crate
/// name (`CARGO_PKG_NAME`) is what a machine types; this is what a person
/// reads.
pub const APP_NAME: &str = "huckleberry-cli";

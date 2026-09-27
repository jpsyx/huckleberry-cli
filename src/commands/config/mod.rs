//! The `config` command: read and change what the tool remembers.
//!
//! One file per action, and this file only routes. The settings themselves,
//! and every rule about what a setting may hold, live in `src/config.rs`.

pub mod set;
pub mod show;

use std::path::Path;

use anyhow::Result;

use crate::cli::ConfigAction;
use crate::config::Config;
use crate::theme::Theme;

/// Runs the requested `config` action against the loaded configuration.
pub fn run(
    action: &ConfigAction,
    path: &Path,
    config: &Config,
    theme: Theme,
    verbose: bool,
) -> Result<()> {
    match action {
        ConfigAction::Show => show::run(config),
        ConfigAction::Set { key, value } => set::run(
            key.as_deref(),
            value.as_deref(),
            path,
            config,
            theme,
            verbose,
        ),
    }
}

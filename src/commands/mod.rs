//! Subcommand dispatch, and the narration every command shares.
//!
//! One thin module per command, each one a shell around a pure function. This
//! file only routes and loads the configuration once, so every command is
//! handed the same settings: it never decides anything a test would want to
//! assert.

pub mod config;
pub mod greet;
pub mod info;

use std::path::PathBuf;

use anyhow::Result;

use crate::cli::{Cli, Command};
use crate::theme::Theme;

/// Runs the command the arguments selected.
pub fn run(cli: &Cli, theme: Theme) -> Result<()> {
    let path = config_path(cli)?;
    let settings = crate::config::load(&path)?;
    let verbose = cli.verbose || settings.verbose;
    match &cli.command {
        Command::Greet { name } => greet::run(name.as_deref(), &settings, theme),
        Command::Info => info::run(&path),
        Command::Config { action } => config::run(action, &path, &settings, theme, verbose),
    }
}

/// The configuration file to use: what `--config` named, else the default one.
fn config_path(cli: &Cli) -> Result<PathBuf> {
    cli.config
        .clone()
        .map_or_else(crate::config::default_path, Ok)
}

/// Says what the tool is about to do, on stderr, always. A person should never
/// wonder whether the tool is hung.
pub fn narrate(theme: Theme, message: &str) {
    eprintln!("{}", theme.info(message));
}

/// Says the same thing in more detail, only under `--verbose` (or the
/// `verbose` setting). Reassurance is the default; detail is opt-in.
pub fn detail(verbose: bool, theme: Theme, message: &str) {
    if verbose {
        eprintln!("{}", theme.muted(message));
    }
}

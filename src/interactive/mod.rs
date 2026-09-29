//! Running one command for the full-screen shell in [`crate::tui`].
//!
//! Navigation moved to that module; what stays here is everything that happens
//! once a row has been chosen: the menu tree it navigates ([`catalog`]), the
//! argument draft a command is built from ([`draft`]), dispatch and its error
//! recovery ([`operation`]), and the session-scoped overrides they all read.
pub mod catalog;
pub mod draft;
pub mod interrupt;
pub mod operation;
pub mod options;
pub mod session;
mod session_menu;
use crate::theme::Theme;
use anyhow::Result;
use session_menu::choose;

/// How the invocation is presented before opening configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMode {
    /// Dispatch an explicitly named command.
    Command,
    /// Start a terminal session.
    Interactive,
    /// Print help without prompting.
    Help,
}

/// Pure entry decision; redirected stderr cannot host prompts.
#[must_use]
pub const fn entry_mode(
    has_command: bool,
    stdin_terminal: bool,
    stderr_terminal: bool,
) -> EntryMode {
    if has_command {
        EntryMode::Command
    } else if stdin_terminal && stderr_terminal {
        EntryMode::Interactive
    } else {
        EntryMode::Help
    }
}

/// Holds static output on screen until it has been read.
pub fn pause(theme: Theme) -> Result<()> {
    choose("Ready?", &["Continue".into()], 0, theme).map(|_| ())
}

/// Reads the long help for one command, chosen from the same tree the menu
/// navigates. It prints rather than drawing, so `--help` output is one thing
/// in this tool rather than two.
pub fn help(theme: Theme) -> Result<()> {
    let mut paths = vec![catalog::CommandPath(vec![])];
    paths.extend(catalog::command_paths());
    let labels = paths
        .iter()
        .map(|path| {
            if path.0.is_empty() {
                "All commands".into()
            } else {
                path.0.join(" ")
            }
        })
        .collect::<Vec<_>>();
    let index = choose("Help for which command?", &labels, 0, theme)?;
    if let Some(mut metadata) = catalog::metadata(&paths[index]) {
        eprintln!("{}", metadata.render_long_help());
    }
    pause(theme)
}

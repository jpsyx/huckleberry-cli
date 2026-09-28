//! placeholder
pub mod cli;
pub mod commands;
pub mod config;
pub mod credentials;
pub mod dashboard;
pub mod dataset;
pub mod domain;
pub mod edit;
pub mod interactive;
pub mod listing;
pub mod prompt;
pub mod render;
pub mod session;
pub mod theme;

pub const APP_NAME: &str = "huckleberry-cli";

/// What the command is called when nothing says otherwise: `hb`, short for
/// huckleberry.
pub const DEFAULT_COMMAND: &str = "hb";

/// How this program was invoked, for the hints in its own failure messages.
///
/// Read from the arguments rather than hardcoded, because `install.sh --name`
/// lets somebody call it whatever they like, and a message telling them to run
/// a command that is not on their PATH is worse than no message.
#[must_use]
pub fn program_name() -> String {
    std::env::args()
        .next()
        .and_then(|path| {
            std::path::Path::new(&path)
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| DEFAULT_COMMAND.to_owned())
}

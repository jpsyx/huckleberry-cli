//! Session-scoped command overrides, separate from persistent settings.
use crate::cli::Cli;
use std::path::PathBuf;

/// Global flags applied to subsequent interactive commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOptions {
    /// Alternate settings and credential location.
    pub config: Option<PathBuf>,
    /// Temporary child override.
    pub child: Option<String>,
    /// Snapshot source; None selects live data.
    pub offline: Option<PathBuf>,
    /// Diagnostic output.
    pub verbose: bool,
}
impl SessionOptions {
    /// Captures global values from the invocation.
    #[must_use]
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            config: cli.config.clone(),
            child: cli.child.clone(),
            offline: cli.offline.clone(),
            verbose: cli.verbose,
        }
    }
}

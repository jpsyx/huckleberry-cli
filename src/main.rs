//! The binary: parse, dispatch, report, exit.
//!
//! Nothing else belongs here. Every decision lives in the library so it can be
//! tested without a process, and failures come back as one `anyhow::Error`
//! that this file turns into a stderr line and a non-zero exit code.

use std::process::ExitCode;

use app::cli::Cli;
use app::theme::Theme;
use clap::Parser;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let theme = Theme::active();
    match app::commands::run(&cli, theme) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            // `{:#}` prints the whole `.context(...)` chain on one line.
            eprintln!("{}", theme.error_line("error:", &format!("{error:#}")));
            ExitCode::FAILURE
        }
    }
}

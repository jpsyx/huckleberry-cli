//! The always-on shell: the screen `h` opens with no command.
//!
//! Split the way the dashboard is, so that nothing which decides anything also
//! owns the terminal: [`keys`] says what a keystroke means, [`state`] holds
//! what is on screen and what a motion does to it, [`draw`] turns that into
//! widgets, and [`shell`] owns the alternate screen and the event loop.
//!
//! The design rules this screen is held to are in
//! [`docs/tui.md`](../../docs/tui.md). The short version: one hand, in the
//! dark, holding a baby.

pub mod data;
pub mod draw;
pub mod facts;
pub mod keys;
pub mod shell;
pub mod state;

pub use draw::draw;
pub use facts::{Facts, Reading};
pub use keys::{Motion, motion_for};
pub use state::{App, Intent, Row};

use anyhow::Result;
use huckleberry_api::client::now_seconds;
use shell::Shell;

use crate::cli::Cli;
use crate::interactive::{interrupt, operation, session::SessionOptions};
use crate::theme::Theme;

/// Opens the shell and keeps it open until the parent leaves.
pub async fn run(cli: &Cli, theme: Theme) -> Result<()> {
    let globals = SessionOptions::from_cli(cli);
    let mut app = App::new();
    let mut shell = Shell::enter()?;
    let outcome = navigate(&mut shell, &mut app, &globals, theme).await;
    // Explicit, so a failure to give the terminal back is reported rather than
    // swallowed by `Drop`.
    let restored = shell.leave();
    outcome.and(restored)
}

/// Draw, collect whatever a background read finished, read one key, act.
async fn navigate(
    shell: &mut Shell,
    app: &mut App,
    globals: &SessionOptions,
    theme: Theme,
) -> Result<()> {
    // The widgets open empty and fill themselves in, so the menu is on screen
    // and usable before the network has answered.
    let mut reading = Some(begin(app, globals, theme));
    loop {
        collect(app, &mut reading).await;
        let height = shell.draw(app, now_seconds())?;
        let Some(motion) = shell.next_motion()? else {
            // A tick with no key. The clocks move on, so draw again.
            continue;
        };
        match app.apply(motion, height) {
            Intent::Stay => {}
            Intent::Quit => return Ok(()),
            Intent::Refresh => {
                if reading.is_none() {
                    reading = Some(begin(app, globals, theme));
                }
            }
            Intent::Version => {
                app.set_status(format!("{} {}", crate::APP_NAME, env!("CARGO_PKG_VERSION")));
            }
            Intent::Help => {
                shell.suspend()?;
                let landing = settle(crate::interactive::help(theme).map(|()| false), theme);
                shell.resume()?;
                land(app, landing);
            }
            Intent::Run(path) => {
                shell.suspend()?;
                let result = interrupt::run(Box::pin(operation::run(&path, globals, theme))).await;
                let landing = settle(result, theme);
                shell.resume()?;
                land(app, landing);
                // A command may have logged the very thing the widgets show,
                // so whatever is in flight describes a moment already gone.
                if let Some(stale) = reading.take() {
                    stale.abort();
                }
                reading = Some(begin(app, globals, theme));
            }
        }
    }
}

/// Starts a read, and says on the screen that one is running.
fn begin(app: &mut App, globals: &SessionOptions, theme: Theme) -> data::Reading {
    app.facts.start_reading();
    data::start(globals, theme)
}

/// Takes on a finished read, if one has finished.
///
/// A failure never takes the numbers away: it is recorded beside them and the
/// widgets keep drawing what they had, because a screen that blanks itself
/// when the wifi drops is worse than one that admits it is showing something
/// from a minute ago.
async fn collect(app: &mut App, reading: &mut Option<data::Reading>) {
    if !reading
        .as_ref()
        .is_some_and(tokio::task::JoinHandle::is_finished)
    {
        return;
    }
    let Some(handle) = reading.take() else {
        return;
    };
    match handle.await {
        Ok(Ok((dataset, calendar, units))) => app.facts.replace(dataset, calendar, units),
        Ok(Err(trouble)) => app.facts.record_trouble(&format!("{trouble:#}")),
        Err(trouble) => app
            .facts
            .record_trouble(&format!("the read stopped: {trouble}")),
    }
}

/// Where the menu goes once a command is finished with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Landing {
    /// A recording finished: start again from the top.
    Home,
    /// A view or a utility finished: stay where it was opened from.
    Stay,
    /// Escape, or Ctrl-C during a wait.
    Cancelled,
    /// It failed, and the failure has already been read.
    Failed,
}

/// Reports how a command ended, with the full screen still out of the way so
/// the failure can be read and scrolled back to.
fn settle(result: Result<bool>, theme: Theme) -> Landing {
    match result {
        Ok(true) => Landing::Home,
        Ok(false) => Landing::Stay,
        Err(error) if crate::prompt::is_cancelled(&error) => Landing::Cancelled,
        Err(error) => {
            eprintln!("{}", theme.error_line("error:", &format!("{error:#}")));
            // A failed write is never replayed on its own; this only holds the
            // message on screen until it has been read.
            let _ = crate::interactive::pause(theme);
            Landing::Failed
        }
    }
}

fn land(app: &mut App, landing: Landing) {
    match landing {
        Landing::Home => app.go_home(),
        Landing::Stay => {}
        Landing::Cancelled => app.set_status("Cancelled."),
        Landing::Failed => app.set_status("That did not work; nothing was retried."),
    }
}

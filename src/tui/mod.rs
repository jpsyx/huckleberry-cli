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
pub mod job;
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
use crate::interactive::{operation, session::SessionOptions};
use crate::theme::Theme;
use job::{Job, Landing};

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

/// Draw, collect what the background has finished, read one key, act.
///
/// One loop, whether a command is running or not. The shell never steps aside:
/// a command draws where the menu was and takes the keys while it is asking,
/// and everything else on the screen keeps going.
async fn navigate(
    shell: &mut Shell,
    app: &mut App,
    globals: &SessionOptions,
    theme: Theme,
) -> Result<()> {
    // The widgets open empty and fill themselves in, so the menu is on screen
    // and usable before the network has answered.
    let mut reading = Some(begin(app, globals, theme));
    let mut job: Option<Job> = None;
    loop {
        collect(app, &mut reading).await;
        if let Some(running) = job.as_mut() {
            running.collect();
        }
        let height = shell.draw_with(app, job.as_ref(), now_seconds())?;

        if job.as_ref().is_some_and(Job::finished) {
            let landing = job
                .take()
                .expect("just checked there is a job")
                .finish()
                .await;
            land(app, landing);
            // A command may have logged the very thing the widgets show, so
            // whatever is in flight describes a moment already gone.
            if let Some(stale) = reading.take() {
                stale.abort();
            }
            reading = Some(begin(app, globals, theme));
            continue;
        }

        let wait = if job.is_some() {
            shell::WORKING
        } else {
            shell::TICK
        };
        let Some(key) = shell.next_key(wait)? else {
            // A tick with no key. The clocks move on, so draw again.
            continue;
        };
        // One key never reaches a command, however deep it is in a question.
        if forces_quit(key) {
            return Ok(());
        }
        if let Some(running) = job.as_mut() {
            if running.asking() {
                running.answer(key);
            }
            continue;
        }

        match app.apply(motion_for(key), height) {
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
            Intent::Help => job = Some(Job::help(theme)),
            Intent::Run(path) if job::takes_the_screen(&path) => {
                // The one command that is a full screen of its own. A terminal
                // has one alternate screen to give, so the shell takes it back
                // when the dashboard is done with it.
                shell.suspend()?;
                let landing =
                    job::settle(Box::pin(operation::run(&path, globals, theme)).await, theme);
                shell.resume()?;
                land(app, landing);
            }
            Intent::Run(path) => job = Some(Job::start(&path, globals, theme)),
        }
    }
}

/// Whether this key ends the session whatever is on the screen.
fn forces_quit(key: crossterm::event::KeyEvent) -> bool {
    motion_for(key) == Motion::Quit
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
        Ok(Ok((dataset, calendar, units, rule))) => {
            app.facts.replace(dataset, calendar, units, rule);
        }
        Ok(Err(trouble)) => app.facts.record_trouble(&format!("{trouble:#}")),
        Err(trouble) => app
            .facts
            .record_trouble(&format!("the read stopped: {trouble}")),
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

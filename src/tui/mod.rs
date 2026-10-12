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
pub mod forms;
pub mod input;
pub mod job;
pub mod keys;
pub mod shell;
pub mod state;
pub mod summary;
pub mod version;
pub mod version_task;

pub use draw::draw;
pub use facts::{Facts, Reading};
pub use keys::{Motion, motion_for};
pub use state::{App, Intent, Row};

use anyhow::Result;
use huckleberry_api::client::now_seconds;
use shell::Shell;

use crate::cli::Cli;
use crate::interactive::session::SessionOptions;
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
    let mut job = Some(Job::startup(globals, theme));
    let mut suspended: Option<forms::Forms> = None;
    let mut version_check = version_task::VersionTask::default();
    loop {
        collect(app, &mut reading).await;
        if let Some(report) = version_check.collect().await {
            app.complete_version(report);
        }
        if let Some(running) = job.as_mut() {
            running.collect(now_seconds()).await;
        }
        let height = shell.draw_with(app, job.as_ref(), now_seconds())?;

        if job.as_ref().is_some_and(Job::finished) {
            let (landing, draft) = job.take().expect("just checked there is a job").finish();
            if shell.discard_pending_input()? {
                return Ok(());
            }
            suspended = draft;
            land(app, landing);
            // A command may have logged the very thing the widgets show, so
            // whatever is in flight describes a moment already gone.
            if let Some(stale) = reading.take() {
                stale.abort();
            }
            reading = Some(begin(app, globals, theme));
            continue;
        }

        let checking_version = app
            .version
            .as_ref()
            .is_some_and(|version| matches!(version.check, version::CheckState::Checking));
        let wait = if job.is_some() || checking_version {
            shell::WORKING
        } else {
            shell::TICK
        };
        let Some(input) = shell.next_input(wait)? else {
            // A tick with nothing typed. The clocks move on, so draw again.
            continue;
        };
        match input::route_with_draft(
            app,
            job.as_mut(),
            &mut suspended,
            input,
            height,
            now_seconds(),
        ) {
            Intent::Stay => {}
            Intent::Quit => return Ok(()),
            Intent::Refresh => {
                if reading.is_none() {
                    reading = Some(begin(app, globals, theme));
                }
            }
            Intent::Version => {
                suspended = None;
                let offline = globals.offline.is_some();
                app.open_version(offline);
                if !offline {
                    version_check.start(crate::version::check(false));
                }
            }
            Intent::CloseVersion => version_check.cancel(),
            Intent::Help => {
                suspended = None;
                job = Some(Job::help(theme));
            }
            Intent::Dashboard => {
                suspended = None;
                app.open_dashboard(days(globals, theme));
            }
            Intent::Run(path) => {
                job = Some(suspended.take().map_or_else(
                    || Job::start(&path, globals, theme),
                    |mut draft| {
                        if draft.resume(&path) {
                            Job::resume(draft, theme)
                        } else {
                            Job::start(&path, globals, theme)
                        }
                    },
                ));
            }
        }
    }
}

/// How many days of history the dashboard covers.
fn days(globals: &SessionOptions, theme: Theme) -> usize {
    crate::interactive::session::load_context(globals, theme)
        .map_or(crate::config::DEFAULT_DAYS, |context| context.days(None)) as usize
}

/// Starts a read, and says on the screen that one is running.
fn begin(app: &mut App, globals: &SessionOptions, theme: Theme) -> data::Reading {
    if let Ok(context) = crate::interactive::session::load_context(globals, theme) {
        app.apply_day_rule(context.config.day_rule());
        app.summary.complete_days = context.days(None);
        app.summary.is_offline = context.offline.is_some();
    }
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
            app.refreshed(dataset, calendar, units, rule);
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

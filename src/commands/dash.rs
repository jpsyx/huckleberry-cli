//! `dash`: the full-screen dashboard's event loop.
//!
//! This module owns the three things the rest of the dashboard deliberately
//! does not: the alternate screen, the keyboard, and the network. Everything
//! it decides lives in [`crate::dashboard::state`].
//!
//! The re-read happens on a timer and on `r`, and a failed re-read never takes
//! the screen away: the numbers that were there stay there and the failure
//! goes on the status bar. A dashboard that blanks itself when the wifi drops
//! is worse than one that admits it is showing something from a minute ago.

use core::time::Duration;

use anyhow::{Context as _, Result, bail};
use crossterm::event::{Event, KeyEventKind};
use huckleberry_api::Huckleberry;
use huckleberry_api::client::now_seconds;

use crate::cli::Units;
use crate::dashboard::{State, draw};
use crate::domain::Calendar;
use crate::session::Context;

/// How long the loop waits for a keystroke before redrawing.
///
/// The live timers count up, so the screen has to redraw on its own; once a
/// second is enough for a clock and cheap enough to be invisible.
const TICK: Duration = Duration::from_secs(1);

/// Runs the dashboard until the person leaves.
pub async fn run(context: &Context, days: Option<u32>, refresh: Option<u32>) -> Result<()> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdout()) {
        bail!("`dash` draws a full screen, so it needs a terminal: try `now` or `summary`");
    }

    let window = context.days(days);
    let every = Duration::from_secs(u64::from(refresh.unwrap_or(context.config.refresh).max(1)));
    let units = Units::from_setting(&context.config.units);

    context.narrate("Reading from Huckleberry...");
    let (dataset, calendar) = super::load(context, Some(window)).await?;
    let mut state =
        State::new(dataset, calendar, window as usize).with_rule(context.config.day_rule());

    // `ratatui::init` also installs a panic hook that restores the terminal,
    // so a panic leaves a usable shell behind rather than a raw-mode mess.
    let mut terminal = ratatui::init();
    let outcome = event_loop(context, &mut terminal, &mut state, units, every, window).await;
    ratatui::restore();
    outcome
}

/// The loop itself, with the terminal already set up.
async fn event_loop(
    context: &Context,
    terminal: &mut ratatui::DefaultTerminal,
    state: &mut State,
    units: Units,
    every: Duration,
    window: u32,
) -> Result<()> {
    let mut last_read = std::time::Instant::now();
    loop {
        terminal
            .draw(|frame| draw(frame, state, units, now_seconds()))
            .context("drawing the dashboard")?;

        if crossterm::event::poll(TICK).context("waiting for a keystroke")?
            && let Event::Key(key) = crossterm::event::read().context("reading a keystroke")?
            // Windows reports both press and release; acting on both would
            // move two tabs for one keystroke.
            && key.kind == KeyEventKind::Press
            && !state.apply(key)
        {
            return Ok(());
        }

        let due = state.refreshing || last_read.elapsed() >= every;
        if due {
            state.refreshing = true;
            terminal
                .draw(|frame| draw(frame, state, units, now_seconds()))
                .context("drawing the dashboard")?;
            reread(context, state, window).await;
            last_read = std::time::Instant::now();
        }
    }
}

/// Re-reads, keeping what is on screen if it fails.
async fn reread(context: &Context, state: &mut State, window: u32) {
    match pull(context, window).await {
        Ok((dataset, calendar)) => {
            state.calendar = calendar;
            state.replace(dataset);
        }
        Err(failure) => state.record_trouble(&format!("{failure:#}")),
    }
}

/// One read, with no narration: the dashboard has a status bar for that.
async fn pull(context: &Context, window: u32) -> Result<(crate::domain::types::Dataset, Calendar)> {
    if let Some(path) = &context.offline {
        let mut dataset = crate::dataset::read_snapshot(path)?;
        context.config.day_rule().apply_to(&mut dataset.child);
        let calendar = Calendar::new(&dataset.timezone)?;
        return Ok((dataset, calendar));
    }
    let client: Huckleberry = context.client()?;
    let cid = super::which_child(context, &client).await?;
    let mut dataset = crate::dataset::pull(
        &client,
        &cid,
        None,
        window,
        &context.config.timezone,
        now_seconds(),
    )
    .await?;
    // The re-read has to keep the family's own night, or the screen would
    // quietly revert to the profile's after the first refresh.
    context.config.day_rule().apply_to(&mut dataset.child);
    super::persist_session(context, &client).await?;
    let calendar = Calendar::new(&dataset.timezone)?;
    Ok((dataset, calendar))
}

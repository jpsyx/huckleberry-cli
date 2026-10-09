//! Pump timer transitions and current status.

use anyhow::Result;
use huckleberry_api::{Huckleberry, TimerChange, client::now_seconds};

use crate::cli::PumpAction;
use crate::prompt;
use crate::render::{format, output};
use crate::session::Context;

/// Dispatches a timer action through explicit event times.
pub(super) async fn run(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &PumpAction,
) -> Result<()> {
    let (change, done, already) = match action {
        PumpAction::Start { start } => {
            let at = prompt::time::read_start(context, start.as_deref())?;
            (
                client.start_pump_at(cid, at).await?,
                "Pumping started.",
                "running",
            )
        }
        PumpAction::Pause { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            (
                client.pause_pump_at(cid, at).await?,
                "Pumping paused.",
                "paused",
            )
        }
        PumpAction::Resume { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            (
                client.resume_pump_at(cid, at).await?,
                "Pumping resumed.",
                "running",
            )
        }
        PumpAction::Cancel => (
            client.cancel_pump(cid).await?,
            "Thrown away. Nothing was recorded.",
            "not running",
        ),
        PumpAction::Status => return status(context, client, cid).await,
        PumpAction::Log { .. } | PumpAction::Stop { .. } => {
            unreachable!("recordings have their own handler")
        }
    };
    report(context, change, done, already);
    Ok(())
}

/// Describes timer changes without treating an idle session as a failure.
fn report(context: &Context, change: TimerChange, done: &str, already: &str) {
    match change {
        TimerChange::Applied => context.report(done),
        TimerChange::NotRunning => context.warn("No pumping session is running."),
        TimerChange::Unchanged => {
            context.warn(&format!("The pumping session is already {already}."));
        }
    }
}

/// Shows whether the timer is running, when it started, and elapsed time.
async fn status(context: &Context, client: &Huckleberry, cid: &str) -> Result<()> {
    let document = client.pump_document(cid).await?;
    let Some(timer) = document
        .as_ref()
        .and_then(huckleberry_api::models::pump::PumpDocument::running_timer)
    else {
        context.present(
            "🧴 Pumping status",
            &[("Status", "No pumping in progress".into())],
            &["running\tfalse".into()],
        );
        return Ok(());
    };
    let paused = timer.paused == Some(true);
    let mut fields = vec![("Status", if paused { "Paused" } else { "Pumping" }.into())];
    let mut machine = vec!["running\ttrue".into(), format!("paused\t{paused}")];
    if let Some(start) = timer.started_at() {
        fields.push(("Started", format::date_time(start, &context.calendar()?)));
        machine.push(format!("started\t{start}"));
    }
    if let Some(elapsed) = timer.elapsed_seconds(now_seconds()) {
        fields.push(("Elapsed", output::duration(elapsed)));
        machine.push(format!("elapsed_seconds\t{elapsed:.0}"));
    }
    context.present("🧴 Pumping status", &fields, &machine);
    Ok(())
}

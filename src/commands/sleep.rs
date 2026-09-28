//! `sleep`: the sleep timer.

use anyhow::Result;
use huckleberry_api::TimerChange;
use huckleberry_api::client::now_seconds;

use crate::cli::SleepAction;
use crate::domain::time::format_duration;
use crate::render::format;
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &SleepAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = match action {
        SleepAction::Start => start(context, &client, &cid).await,
        SleepAction::Pause => {
            report(
                context,
                client.pause_sleep(&cid).await?,
                "Paused.",
                "paused",
            );
            Ok(())
        }
        SleepAction::Resume => {
            report(
                context,
                client.resume_sleep(&cid).await?,
                "Resumed.",
                "running",
            );
            Ok(())
        }
        SleepAction::Cancel => {
            report(
                context,
                client.cancel_sleep(&cid).await?,
                "Thrown away. Nothing was recorded.",
                "not running",
            );
            Ok(())
        }
        SleepAction::Stop => stop(context, &client, &cid).await,
        SleepAction::Status => status(context, &client, &cid).await,
    };
    super::persist_session(context, &client).await?;
    outcome
}

async fn start(context: &Context, client: &huckleberry_api::Huckleberry, cid: &str) -> Result<()> {
    // Starting a second sleep would leave the first unrecorded, which is the
    // one mistake here that loses data rather than just being noisy.
    if let Some(timer) = client
        .sleep_document(cid)
        .await?
        .and_then(|document| document.timer)
        && timer.active
    {
        let running = timer
            .elapsed(now_seconds())
            .map_or_else(String::new, |seconds| {
                format!(" ({})", format_duration(seconds))
            });
        context.warn(&format!(
            "A sleep is already running{running}. Use `sleep stop` to finish it."
        ));
        return Ok(());
    }
    client.start_sleep(cid).await?;
    context.report("Sleep started.");
    Ok(())
}

async fn stop(context: &Context, client: &huckleberry_api::Huckleberry, cid: &str) -> Result<()> {
    if let Some(completed) = client.complete_sleep(cid).await? {
        let calendar = context.calendar()?;
        context.report(&format!(
            "Slept {} · from {}",
            format_duration(completed.duration as f64),
            format::clock(completed.start as f64, &calendar)
        ));
        println!("duration_seconds\t{}", completed.duration);
        println!("start\t{}", completed.start);
    } else {
        context.warn("No sleep was running.");
    }
    Ok(())
}

async fn status(context: &Context, client: &huckleberry_api::Huckleberry, cid: &str) -> Result<()> {
    let document = client.sleep_document(cid).await?;
    let at = now_seconds();
    let calendar = context.calendar()?;

    let Some(timer) = document
        .as_ref()
        .and_then(|document| document.timer.as_ref())
    else {
        println!("running\tfalse");
        context.detail("no sleep timer on this child yet");
        return Ok(());
    };

    println!("running\t{}", timer.active);
    println!("paused\t{}", timer.paused);
    if let Some(started) = timer.started_at() {
        println!("started\t{}", started as i64);
        println!("started_clock\t{}", format::clock(started, &calendar));
    }
    if let Some(seconds) = timer.elapsed(at) {
        println!("elapsed_seconds\t{}", seconds as i64);
        let paused = if timer.paused { " (paused)" } else { "" };
        context.report(&format!("Asleep {}{paused}.", format_duration(seconds)));
    } else {
        let last = document
            .as_ref()
            .and_then(|document| document.prefs.as_ref())
            .and_then(|prefs| prefs.last_sleep.as_ref())
            .and_then(|sleep| sleep.duration);
        context.detail(&last.map_or_else(
            || "not asleep".to_owned(),
            |duration| {
                format!(
                    "not asleep; the last sleep was {}",
                    format_duration(duration.as_f64())
                )
            },
        ));
    }
    Ok(())
}

/// Turns a timer change into the line a person reads.
fn report(context: &Context, change: TimerChange, done: &str, already: &str) {
    match change {
        TimerChange::Applied => context.report(done),
        TimerChange::NotRunning => context.warn("No sleep is running."),
        TimerChange::Unchanged => context.warn(&format!("The sleep is already {already}.")),
    }
}

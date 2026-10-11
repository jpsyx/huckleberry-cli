//! `sleep`: the sleep timer.

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Huckleberry, TimerChange};

use crate::cli::{Overlap, SleepAction};
use crate::domain::time::format_duration;
use crate::prompt::Choice;
use crate::render::{format, output};
use crate::session::Context;

pub(crate) mod prepare;
use super::sleep_form::{SleepOperation, SleepSubmission};

/// Runs the same retained form as the shell, then consumes its submission.
pub async fn run(context: &Context, action: &SleepAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = if matches!(action, SleepAction::Status) {
        status(context, &client, &cid).await
    } else {
        let answer = prepare::collect(context, &client, &cid, action).await?;
        execute(context, &client, &cid, answer).await
    };
    super::persist_session(context, &client).await?;
    outcome
}

async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: SleepSubmission,
) -> Result<()> {
    let at = answer.start.unwrap_or_default();
    match answer.operation {
        SleepOperation::Start => start(context, client, cid, at).await,
        SleepOperation::Manual => manual(context, client, cid, answer).await,
        SleepOperation::Stop => stop(context, client, cid, at).await,
        SleepOperation::Pause => {
            report(
                context,
                client.pause_sleep_at(cid, at).await?,
                "😴 Sleep paused.",
                "paused",
            );
            Ok(())
        }
        SleepOperation::Resume => {
            report(
                context,
                client.resume_sleep_at(cid, at).await?,
                "😴 Sleep resumed.",
                "running",
            );
            Ok(())
        }
        SleepOperation::Cancel => {
            report(
                context,
                client.cancel_sleep(cid).await?,
                "Thrown away. Nothing was recorded.",
                "not running",
            );
            Ok(())
        }
    }
}

async fn start(context: &Context, client: &Huckleberry, cid: &str, began: f64) -> Result<()> {
    // Check after prompting: another caregiver may start a timer while the
    // question is open.
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
    client.start_sleep_at(cid, began).await?;
    context.report("😴 Sleep started.");
    Ok(())
}

/// Validates current timer identity before applying an already collected overlap decision.
async fn manual(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: SleepSubmission,
) -> Result<()> {
    let calendar = context.calendar()?;
    let began = answer.start.expect("validated start");
    let ended = answer.end.expect("validated end");
    let current = client
        .sleep_document(cid)
        .await?
        .and_then(|document| document.timer);
    if current != answer.running {
        bail!("The sleep timer changed while this entry was open. Start again to review it.");
    }
    if let Some(overlap) = answer.overlap
        && !resolve_overlap(context, client, cid, overlap).await?
    {
        return Ok(());
    }
    let duration = ended - began;
    client.log_sleep(cid, began, duration).await?;
    context.receipt(
        "😴 Sleep recorded",
        &output::sleep_fields(began, duration, &calendar),
        &[
            format!("start\t{began:.0}"),
            format!("duration_seconds\t{duration:.0}"),
        ],
    );
    Ok(())
}

/// Does what was decided about the overlap, and says whether the manual entry
/// still stands.
async fn resolve_overlap(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: Overlap,
) -> Result<bool> {
    match answer {
        Overlap::DiscardSleep => {
            client.cancel_sleep(cid).await?;
            context.report("Threw away the sleep in progress. Nothing was recorded for it.");
            Ok(true)
        }
        Overlap::DiscardManual => {
            context.report("Kept the sleep in progress. Nothing was added.");
            Ok(false)
        }
        Overlap::RecordSleep => {
            match client.complete_sleep(cid).await? {
                Some(completed) => context.report(&format!(
                    "Finished the sleep in progress and recorded {}. Nothing was added.",
                    format_duration(completed.duration as f64)
                )),
                None => context.warn("There was nothing to record after all."),
            }
            Ok(false)
        }
    }
}

async fn stop(
    context: &Context,
    client: &huckleberry_api::Huckleberry,
    cid: &str,
    at: f64,
) -> Result<()> {
    let calendar = context.calendar()?;
    if let Some(completed) = client.complete_sleep_at(cid, at).await? {
        context.receipt(
            "😴 Sleep recorded",
            &output::sleep_fields(completed.start as f64, completed.duration as f64, &calendar),
            &[
                format!("duration_seconds\t{}", completed.duration),
                format!("start\t{}", completed.start),
            ],
        );
    } else {
        context.warn("No sleep was running.");
    }
    Ok(())
}

async fn status(context: &Context, client: &huckleberry_api::Huckleberry, cid: &str) -> Result<()> {
    let document = client.sleep_document(cid).await?;
    let at = now_seconds();
    let calendar = context.calendar()?;

    let timer = document
        .as_ref()
        .and_then(|document| document.timer.as_ref());
    let last = document
        .as_ref()
        .and_then(|document| document.prefs.as_ref())
        .and_then(|prefs| prefs.last_sleep.as_ref())
        .and_then(|sleep| sleep.duration)
        .map(huckleberry_api::models::Number::as_f64);
    let fields = output::sleep_status_fields(timer, last, at, &calendar);
    let Some(timer) = timer else {
        context.present("😴 Sleep status", &fields, &["running\tfalse".into()]);
        return Ok(());
    };
    let mut machine = vec![
        format!("running\t{}", timer.active),
        format!("paused\t{}", timer.paused),
    ];
    if let Some(started) = timer.started_at() {
        machine.push(format!("started\t{}", started as i64));
        machine.push(format!(
            "started_clock\t{}",
            format::clock(started, &calendar)
        ));
    }
    if let Some(seconds) = timer.elapsed(at) {
        machine.push(format!("elapsed_seconds\t{}", seconds as i64));
    }
    context.present("😴 Sleep status", &fields, &machine);
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

/// Choices for overlap resolution, beginning with a non-mutating escape.
#[must_use]
pub const fn overlap_choices() -> &'static [Choice<'static>] {
    const CHOICES: [Choice<'static>; 4] = [
        Choice {
            value: "cancel",
            hint: "Cancel without changing either sleep",
        },
        Choice {
            value: "discard-sleep",
            hint: "Throw away the sleep in progress, and keep this entry",
        },
        Choice {
            value: "discard-manual",
            hint: "Keep the sleep in progress, and throw away this entry",
        },
        Choice {
            value: "record-sleep",
            hint: "Finish and record the sleep in progress, and throw away this entry",
        },
    ];
    &CHOICES
}

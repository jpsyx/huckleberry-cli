//! `sleep`: the sleep timer.

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Huckleberry, TimerChange};

use crate::cli::{Overlap, SleepAction};
use crate::domain::clock;
use crate::domain::time::format_duration;
use crate::prompt::time::read_clock;
use crate::prompt::{self, Choice, Question};
use crate::render::{format, output};
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &SleepAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = match action {
        SleepAction::Start { start: given } => {
            start(context, &client, &cid, given.as_deref()).await
        }
        SleepAction::Manual {
            start,
            end,
            overlap,
        } => {
            manual(
                context,
                &client,
                &cid,
                start.as_deref(),
                end.as_deref(),
                *overlap,
            )
            .await
        }
        SleepAction::Pause { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            report(
                context,
                client.pause_sleep_at(&cid, at).await?,
                "😴 Sleep paused.",
                "paused",
            );
            Ok(())
        }
        SleepAction::Resume { at } => {
            let at = prompt::time::read_at(context, at.as_deref())?;
            report(
                context,
                client.resume_sleep_at(&cid, at).await?,
                "😴 Sleep resumed.",
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
        SleepAction::Stop { at } => stop(context, &client, &cid, at.as_deref()).await,
        SleepAction::Status => status(context, &client, &cid).await,
    };
    super::persist_session(context, &client).await?;
    outcome
}

async fn start(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    given: Option<&str>,
) -> Result<()> {
    let began = prompt::time::read_start(context, given)?;
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

/// Records a sleep that has already happened.
///
/// Two questions, because that is all a sleep is: when it began and when it
/// ended. Neither carries a date. A bare time is the most recent one, so at
/// 3am `11:30pm` is last night, and the end is the first such time after the
/// start, so a sleep across midnight needs nobody to say so.
///
/// A sleep in progress is not in the way: the entry is history and the timer
/// is a timer. They only meet when the entry runs into the running sleep, and
/// then the person says which of the two to keep, because all three answers
/// throw something away and none of them is a default.
async fn manual(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    start: Option<&str>,
    end: Option<&str>,
    overlap: Option<Overlap>,
) -> Result<()> {
    let calendar = context.calendar()?;
    let now = now_seconds();

    let began = clock::most_recent(
        read_clock(context, start, "When did it begin?", "--start <TIME>")?,
        now,
        &calendar,
    );
    let ended = clock::first_after(
        read_clock(context, end, "When did it end?", "--end <TIME>")?,
        began,
        &calendar,
    );

    let duration = ended - began;
    if duration <= 0.0 {
        bail!("a sleep has to last: that one begins and ends at the same moment");
    }
    if ended > now {
        bail!(
            "that sleep ends at {}, which is still to come: record it when it has finished",
            format::clock(ended, &calendar)
        );
    }

    // A paused sleep ends at the moment it was paused, not now, which is the
    // same rule `complete_sleep` records it by.
    let running = client
        .sleep_document(cid)
        .await?
        .and_then(|document| document.timer)
        .filter(|timer| timer.active)
        .and_then(|timer| {
            let started = timer.started_at()?;
            let finished = if timer.paused {
                timer.paused_at().unwrap_or(now)
            } else {
                now
            };
            Some((started, finished))
        });
    if let Some((running_start, running_end)) = running
        && clock::overlaps(began, ended, running_start, running_end)
    {
        let answer = match overlap {
            Some(given) => given,
            None => ask_about_overlap(context, running_start, now, &calendar)?,
        };
        if !resolve_overlap(context, client, cid, answer).await? {
            return Ok(());
        }
    }

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

/// Asks what to do about an entry that runs into the running sleep.
///
/// The warning is `attention`, not `error`: nothing has gone wrong, and this
/// is a screen about somebody's baby. Every answer says what it throws away,
/// because that is the part worth reading twice at 3am.
fn ask_about_overlap(
    context: &Context,
    running_start: f64,
    now: f64,
    calendar: &crate::domain::Calendar,
) -> Result<Overlap> {
    context.attention(&format!(
        "That overlaps the sleep in progress, which started at {} and has run {}.",
        format::clock(running_start, calendar),
        format_duration(now - running_start)
    ));
    let question = Question::new(
        "what to do about the overlap",
        "Which one do you want to keep?",
        "--overlap <CHOICE>",
    )
    .with_choices(overlap_choices());
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "cancel" => return Err(prompt::Cancelled.into()),
        "discard-manual" => Overlap::DiscardManual,
        "record-sleep" => Overlap::RecordSleep,
        _ => Overlap::DiscardSleep,
    })
}

async fn stop(
    context: &Context,
    client: &huckleberry_api::Huckleberry,
    cid: &str,
    at: Option<&str>,
) -> Result<()> {
    let calendar = context.calendar()?;
    let document = client.sleep_document(cid).await?;
    if let Some(message) = output::sleep_start_context(
        document
            .as_ref()
            .and_then(|document| document.timer.as_ref()),
        now_seconds(),
        &calendar,
    ) {
        context.narrate(&message);
    }
    let at = prompt::time::read_end(context, at)?;
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

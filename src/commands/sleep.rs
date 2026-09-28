//! `sleep`: the sleep timer.

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Huckleberry, TimerChange};

use crate::cli::{Overlap, SleepAction};
use crate::domain::clock::{self, TimeOfDay, Typed};
use crate::domain::time::format_duration;
use crate::prompt::{self, Choice, Question};
use crate::render::format;
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &SleepAction) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let outcome = match action {
        SleepAction::Start => start(context, &client, &cid).await,
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
        read_time(context, start, "When did it begin?", "--start <TIME>")?,
        now,
        &calendar,
    );
    let ended = clock::first_after(
        read_time(context, end, "When did it end?", "--end <TIME>")?,
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
    context.report(&format!(
        "Recorded {} of sleep · {} to {}",
        format_duration(duration),
        format::clock(began, &calendar),
        format::clock(ended, &calendar)
    ));
    println!("start\t{began:.0}");
    println!("duration_seconds\t{duration:.0}");
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
    const CHOICES: [Choice<'static>; 3] = [
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
    .with_choices(&CHOICES);
    Ok(match prompt::ask(&question, context.theme)?.as_str() {
        "discard-manual" => Overlap::DiscardManual,
        "record-sleep" => Overlap::RecordSleep,
        _ => Overlap::DiscardSleep,
    })
}

/// One time, from the flag or from a question, asking again when what was
/// typed is not a time and asking which half of the day when it is both.
fn read_time(context: &Context, given: Option<&str>, label: &str, flag: &str) -> Result<TimeOfDay> {
    if let Some(text) = given {
        return match clock::parse(text) {
            Some(Typed::Certain(time)) => Ok(time),
            Some(Typed::Ambiguous { .. }) => {
                bail!("`{text}` is either half of the day: say `{text}am` or `{text}pm`")
            }
            None => bail!("`{text}` is not a time I can read: try `9pm`, `21:00` or `0357`"),
        };
    }
    let question = Question::new("time", label, flag);
    loop {
        let answer = prompt::ask(&question, context.theme)?;
        match clock::parse(&answer) {
            Some(Typed::Certain(time)) => return Ok(time),
            Some(Typed::Ambiguous { morning, afternoon }) => {
                return ask_which_half(context, morning, afternoon);
            }
            None => context.warn(&format!(
                "`{answer}` is not a time I can read: try `9pm`, `21:00` or `0357`"
            )),
        }
    }
}

/// Asks which half of the day a bare time meant.
fn ask_which_half(
    context: &Context,
    morning: TimeOfDay,
    afternoon: TimeOfDay,
) -> Result<TimeOfDay> {
    let morning_label = morning.label();
    let afternoon_label = afternoon.label();
    let choices = [
        Choice {
            value: "am",
            hint: &morning_label,
        },
        Choice {
            value: "pm",
            hint: &afternoon_label,
        },
    ];
    let question = Question::new(
        "half of the day",
        "Which one?",
        "--start <TIME> with am or pm",
    )
    .with_choices(&choices);
    Ok(if prompt::ask(&question, context.theme)? == "pm" {
        afternoon
    } else {
        morning
    })
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

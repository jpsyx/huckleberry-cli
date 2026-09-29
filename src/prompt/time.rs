//! Shared time questions for manual entries and running timers.

use std::io::IsTerminal;

use anyhow::{Result, anyhow, bail};
use huckleberry_api::client::now_seconds;

use super::{Choice, Question};
use crate::domain::clock::{self, TimeOfDay, Typed};
use crate::domain::time::Calendar;
use crate::session::Context;

/// Examples for questions accepting both clock times and relative minutes.
const TIME_HELP: &str = "E.g. '1:23 pm' or '123pm' or '32 min ago' are all valid";

/// Reads a required manual-sleep start, preserving the date of relative answers.
pub fn read_manual_start(context: &Context, given: Option<&str>) -> Result<f64> {
    let question = create_time_question("When did it begin?", "--start <TIME>");
    read(context, given, &question, |text, interactive| {
        parse_instant(context, text, interactive, clock::most_recent)
    })
}

/// Reads a required manual end; only clock-only answers roll forward after the start.
pub fn read_manual_end(context: &Context, given: Option<&str>, started: f64) -> Result<f64> {
    let question = create_time_question("When did it end?", "--end <TIME>");
    read(context, given, &question, |text, interactive| {
        parse_instant(context, text, interactive, |time, _, calendar| {
            clock::first_after(time, started, calendar)
        })
    })
}

/// Builds every time question with the same examples and muted helper style.
const fn create_time_question<'a>(label: &'a str, flag: &'a str) -> Question<'a> {
    Question::new("time", label, flag).with_help(TIME_HELP)
}

/// Reads a timer start, offering now and accepting relative minutes or a clock time.
/// Without a terminal, an omitted value retains the default of now.
pub fn read_start(context: &Context, given: Option<&str>) -> Result<f64> {
    read_instant(context, given, "When did it start?", "--start <TIME>")
}

/// Reads a timer end using the same defaults, formatting and parser as its start.
pub fn read_end(context: &Context, given: Option<&str>) -> Result<f64> {
    read_instant(context, given, "When did it end?", "--at <TIME>")
}

/// Reads when an event happened, using the same defaults and parser as sleep starts.
pub fn read_at(context: &Context, given: Option<&str>) -> Result<f64> {
    read_instant(context, given, "When?", "--at <TIME>")
}

/// Resolves an answer after it is read, so accepting now means the current instant.
fn read_instant(context: &Context, given: Option<&str>, label: &str, flag: &str) -> Result<f64> {
    let question = create_time_question(label, flag).with_default("now");
    let given = given.or_else(|| (!std::io::stdin().is_terminal()).then_some("now"));
    read(context, given, &question, |text, interactive| {
        parse_instant(context, text, interactive, clock::most_recent)
    })
}

/// Reads a history time, preserving the original instant when its default is kept.
/// Clock-only answers stay on the entry's local date; dated answers can move days.
pub fn read_edit_at(context: &Context, given: Option<&str>, current: f64) -> Result<f64> {
    let started = read_existing(
        context,
        given,
        current,
        "When?",
        "--set at=<TIME>",
        move |time, _, calendar| calendar.at(calendar.day_of(current), time.hour, time.minute),
    )?;
    if started.to_bits() != current.to_bits()
        && (!started.is_finite() || started < 0.0 || started > now_seconds())
    {
        bail!("choose a time between the Unix epoch and now");
    }
    Ok(started)
}

/// Reads a history sleep's stop, defaulting to where the entry ends now.
///
/// A clock-only answer lands on the first such time after the start, the way a
/// manual sleep's end does, so an overnight stop needs no date. Dated and
/// relative answers are taken as they are given.
///
/// # Errors
///
/// When the answer is not a time, or is not after the start: a sleep that ends
/// before it begins is not a duration anybody can store.
pub fn read_edit_stop(
    context: &Context,
    given: Option<&str>,
    started: f64,
    current: f64,
) -> Result<f64> {
    let stopped = read_existing(
        context,
        given,
        current,
        "When did it end?",
        "--set duration=<MINUTES>",
        move |time, _, calendar| clock::first_after(time, started, calendar),
    )?;
    if stopped <= started {
        bail!("a sleep ends after it starts: choose a time after it began");
    }
    Ok(stopped)
}

/// Shares existing-time defaults between history and live timer corrections.
///
/// `resolve_clock` decides what a clock-only answer means: the entry's own day
/// for a history time, the most recent occurrence for a timer, the first one
/// after the start for a stop.
fn read_existing(
    context: &Context,
    given: Option<&str>,
    current: f64,
    label: &str,
    flag: &str,
    resolve_clock: impl Fn(TimeOfDay, f64, &Calendar) -> f64 + Copy,
) -> Result<f64> {
    let calendar = context.calendar()?;
    let shown = calendar.zoned(current).strftime(DATED_FORMAT).to_string();
    let question = create_time_question(label, flag).with_default(&shown);
    read(context, given, &question, |text, interactive| {
        if text.trim() == shown || text.trim().eq_ignore_ascii_case("keep") {
            return Ok(Some(current));
        }
        if let Some(dated) = parse_dated(text.trim()) {
            let zoned = dated.in_tz(calendar.name())?;
            return Ok(Some(
                zoned.timestamp().as_second() as f64
                    + f64::from(zoned.timestamp().subsec_nanosecond()) / 1e9,
            ));
        }
        parse_instant(context, text, interactive, resolve_clock)
    })
}

/// How a date and time are shown and read back: `2025-01-01 7:00 AM`.
const DATED_FORMAT: &str = "%Y-%m-%d %-I:%M %p";

/// Reads a dated answer, both as the default is shown and in plain ISO order.
fn parse_dated(text: &str) -> Option<jiff::civil::DateTime> {
    jiff::civil::DateTime::strptime(DATED_FORMAT, text)
        .ok()
        .or_else(|| text.parse().ok())
}

/// Asks for a replacement start; `None` means keep the timer untouched on Enter.
/// Scripts must supply a value instead of silently resetting the timer to now.
pub fn read_edit_start(
    context: &Context,
    given: Option<&str>,
    current: Option<f64>,
) -> Result<Option<f64>> {
    if let Some(original) = current {
        let changed = read_existing(
            context,
            given,
            original,
            "When?",
            "--set start=<TIME>",
            clock::most_recent,
        )?;
        return Ok((changed.to_bits() != original.to_bits()).then_some(changed));
    }
    let calendar = context.calendar()?;
    let current = current.map_or_else(
        || "not recorded".into(),
        |at| crate::render::format::date_time(at, &calendar),
    );
    let label = format!("New start time? Current: {current}");
    let question = create_time_question(&label, "--set start=<TIME>").with_default("keep");
    read(context, given, &question, |text, interactive| {
        if text.trim().eq_ignore_ascii_case("keep") {
            return Ok(Some(None));
        }
        Ok(parse_instant(context, text, interactive, clock::most_recent)?.map(Some))
    })
}

/// Parses all activity times; callers supply only the date rule for clock-only answers.
fn parse_instant(
    context: &Context,
    text: &str,
    interactive: bool,
    resolve_clock: impl FnOnce(TimeOfDay, f64, &Calendar) -> f64,
) -> Result<Option<f64>> {
    let now = now_seconds();
    if let Some(started) = clock::parse_relative(text, now) {
        if started < 0.0 {
            bail!("choose a time between the Unix epoch and now");
        }
        return Ok(Some(started));
    }
    let calendar = context.calendar()?;
    Ok(parse_clock(context, text, interactive)?
        .map(|time| resolve_clock(time, now_seconds(), &calendar)))
}

/// Supplied values fail immediately; prompted values can be corrected before writing.
fn read<T>(
    context: &Context,
    given: Option<&str>,
    question: &Question<'_>,
    parse: impl Fn(&str, bool) -> Result<Option<T>>,
) -> Result<T> {
    if let Some(text) = given {
        return parse(text, false)?.ok_or_else(|| anyhow!(unreadable_message(text)));
    }
    loop {
        let answer = super::ask(question, context.theme)?;
        match parse(&answer, true)? {
            Some(value) => return Ok(value),
            None => context.warn(&unreadable_message(&answer)),
        }
    }
}

/// Interprets a clock time, resolving ambiguity only when the user typed it interactively.
fn parse_clock(context: &Context, text: &str, interactive: bool) -> Result<Option<TimeOfDay>> {
    match clock::parse(text) {
        Some(Typed::Certain(time)) => Ok(Some(time)),
        Some(Typed::Ambiguous { morning, afternoon }) if interactive => {
            ask_which_half(context, morning, afternoon).map(Some)
        }
        Some(Typed::Ambiguous { .. }) => {
            bail!("`{text}` is either half of the day: say `{text}am` or `{text}pm`")
        }
        None => Ok(None),
    }
}

/// The same examples appear in initial hints and failed parsing guidance.
fn unreadable_message(text: &str) -> String {
    format!("`{text}` is not a time I can read. {TIME_HELP}")
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
    Ok(if super::ask(&question, context.theme)? == "pm" {
        afternoon
    } else {
        morning
    })
}

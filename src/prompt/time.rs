//! Shared time questions for manual entries and running timers.

use std::io::IsTerminal;

use anyhow::{Result, anyhow, bail};
use huckleberry_api::client::now_seconds;

use super::{Choice, Question};
use crate::domain::clock::{self, TimeOfDay, Typed};
use crate::session::Context;

/// Reads a clock time, asking which half of the day an ambiguous answer meant.
pub fn read_clock(
    context: &Context,
    given: Option<&str>,
    label: &str,
    flag: &str,
) -> Result<TimeOfDay> {
    let question = Question::new("time", label, flag);
    read(context, given, &question, |text, interactive| {
        parse_clock(context, text, interactive)
    })
}

/// Reads a timer start, offering now and accepting relative minutes or a clock time.
/// Without a terminal, an omitted value retains the default of now.
pub fn read_start(context: &Context, given: Option<&str>) -> Result<f64> {
    let calendar = context.calendar()?;
    let question = Question::new(
        "start time",
        "When did it start? (e.g. 358 am or 10 minutes ago)",
        "--start <TIME>",
    )
    .with_default("now");
    let given = given.or_else(|| (!std::io::stdin().is_terminal()).then_some("now"));
    read(context, given, &question, |text, interactive| {
        let now = now_seconds();
        if let Some(started) = clock::parse_relative(text, now) {
            return Ok(Some(started));
        }
        Ok(parse_clock(context, text, interactive)?
            .map(|time| clock::most_recent(time, now_seconds(), &calendar)))
    })
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

/// Examples shared by both time readers when a value cannot be parsed.
fn unreadable_message(text: &str) -> String {
    format!("`{text}` is not a time I can read: try `9pm`, `21:00` or `0357`")
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

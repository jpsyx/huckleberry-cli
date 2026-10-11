//! Shared question definitions for terminal and shell diaper collection.
use crate::{
    domain::Calendar,
    prompt::{
        dial::time::{Mode, Opening},
        flow::{FormValue, View, time::TimeState},
    },
};
use anyhow::{Result, bail};

pub(super) fn create_view(
    field: &str,
    potty: bool,
    calendar: &Calendar,
    at: f64,
    now: f64,
) -> Result<View> {
    Ok(match field {
        "at" => View::create_time(
            "When?",
            TimeState::create_time(
                calendar.clone(),
                Opening {
                    at,
                    mode: Mode::Relative,
                    not_before: None,
                },
                now,
            ),
        ),
        "mode" => choices(
            if potty {
                "What happened?"
            } else {
                "What was in it?"
            },
            &[
                ("pee", "Wet only"),
                ("poo", "Dirty only"),
                ("both", "Both"),
                ("dry", "Neither"),
            ],
            false,
        ),
        "outcome" => choices(
            "How did it go?",
            &[
                ("went-potty", "Went"),
                ("sat-but-dry", "Sat, but nothing happened"),
                ("accident", "Did not make it"),
            ],
            false,
        ),
        "pee" | "poo" => choices(
            if field == "pee" {
                "How much wet?"
            } else {
                "How much dirty?"
            },
            &[
                ("little", "A little"),
                ("medium", "A medium amount"),
                ("big", "A lot"),
            ],
            true,
        ),
        "colour" => choices(
            "What colour?",
            &[
                ("yellow", "Yellow"),
                ("brown", "Brown"),
                ("green", "Green"),
                ("black", "Black"),
                ("red", "Red"),
                ("gray", "Grey"),
            ],
            true,
        ),
        "consistency" => choices(
            "What consistency?",
            &[
                ("loose", "Loose"),
                ("runny", "Runny"),
                ("solid", "Solid"),
                ("mucousy", "Mucousy"),
                ("hard", "Hard"),
                ("pebbles", "Pebbles"),
                ("diarrhea", "Diarrhea"),
            ],
            true,
        ),
        "rash" => View::create_choices(
            "Any rash?",
            vec![
                ("Yes".into(), FormValue::Choice("yes".into())),
                ("No".into(), FormValue::Choice("no".into())),
            ],
            1,
        ),
        "notes" => View::create_text("Anything to note?", "", false),
        _ => bail!("unknown diaper field: {field}"),
    })
}
fn choices(label: &str, values: &[(&str, &str)], optional: bool) -> View {
    let mut rows = Vec::new();
    if optional {
        rows.push(("Skip".into(), FormValue::Skipped));
    }
    rows.extend(
        values
            .iter()
            .map(|(value, label)| ((*label).into(), FormValue::Choice((*value).into()))),
    );
    View::create_choices(label, rows, 0)
}

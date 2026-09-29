//! Stored edit-field values and their finite alternatives.
use crate::cli::{
    Amount, BottleKind, Colour, Consistency, DiaperKind, PottyOutcome, Reaction, Units,
};
use crate::domain::time::format_duration;
use crate::edit::{DiaperDraft, Draft};
use clap::ValueEnum;

/// All accepted stored values for a finite history-edit field.
#[must_use]
pub fn choices(field: &str) -> Vec<(String, String)> {
    macro_rules! values {
        ($kind:ty) => {
            <$kind>::value_variants()
                .iter()
                .map(|value| {
                    let stored = value.to_api().as_str().to_owned();
                    (stored.clone(), crate::render::output::words(&stored))
                })
                .collect()
        };
    }
    match field {
        "mode" => values!(DiaperKind),
        "pee" | "poo" => values!(Amount),
        "color" => values!(Colour),
        "consistency" => values!(Consistency),
        "type" => values!(BottleKind),
        "units" => values!(Units),
        "how" => values!(PottyOutcome),
        "reaction" => values!(Reaction),
        "rash" => vec![("true".into(), "Yes".into()), ("false".into(), "No".into())],
        _ => vec![],
    }
}

/// How a field nobody has filled in reads in the picker.
const NOT_SET: &str = "not set";

/// What a field holds now, in the words the edit picker shows it in.
///
/// The picker lists the value each field would be saved with, so a parent can
/// see what "keep" would keep without opening the field.
#[must_use]
pub fn current(draft: &Draft, field: &str) -> String {
    match draft {
        Draft::Diaper(diaper) => diaper_value(diaper, field),
        Draft::Bottle(bottle) => match field {
            "amount" => format!(
                "{} {}",
                crate::render::format::amount_in(bottle.amount, bottle.units),
                bottle.units.as_str()
            ),
            "units" => bottle.units.as_str().to_owned(),
            "type" => spelled(bottle.kind.to_api().as_str()),
            _ => written(bottle.notes.as_deref()),
        },
        Draft::Nursing(nursing) => match field {
            "left" => format_duration(nursing.left_minutes * 60.0),
            "right" => format_duration(nursing.right_minutes * 60.0),
            _ => written(nursing.notes.as_deref()),
        },
        Draft::Solids(meal) => match field {
            "foods" => meal.foods.join(", "),
            "amount" => meal.amount.clone(),
            "reaction" => {
                optional_word(meal.reaction.map(|value| spelled(value.to_api().as_str())))
            }
            _ => written(meal.notes.as_deref()),
        },
        Draft::Sleep(sleep) => match field {
            "duration" => format_duration(sleep.minutes * 60.0),
            _ => written(sleep.notes.as_deref()),
        },
    }
}

fn diaper_value(diaper: &DiaperDraft, field: &str) -> String {
    match field {
        "mode" => spelled(diaper.mode.to_api().as_str()),
        "pee" => optional_word(diaper.pee.map(|value| spelled(value.to_api().as_str()))),
        "poo" => optional_word(diaper.poo.map(|value| spelled(value.to_api().as_str()))),
        "color" => optional_word(diaper.color.map(|value| spelled(value.to_api().as_str()))),
        "consistency" => optional_word(
            diaper
                .consistency
                .map(|value| spelled(value.to_api().as_str())),
        ),
        "how" => optional_word(diaper.how.map(|value| spelled(value.to_api().as_str()))),
        "rash" => if diaper.rash { "Yes" } else { "No" }.to_owned(),
        _ => written(diaper.notes.as_deref()),
    }
}

/// A spelled-out value, or the words for a field nobody filled in.
fn optional_word(spelled: Option<String>) -> String {
    spelled.unwrap_or_else(|| NOT_SET.to_owned())
}

/// Whatever a parent typed, or nothing when the field is empty.
fn written(text: Option<&str>) -> String {
    text.filter(|text| !text.trim().is_empty())
        .map_or_else(|| NOT_SET.to_owned(), str::to_owned)
}

/// The app's own spelling in this tool's words, as the choice lists show it.
fn spelled(stored: &str) -> String {
    crate::render::output::words(stored)
}

/// Whether clearing an edit field has a defined meaning.
#[must_use]
pub fn optional(field: &str) -> bool {
    matches!(
        field,
        "pee" | "poo" | "color" | "consistency" | "how" | "reaction" | "notes"
    )
}

/// Replaces a pending field edit; Keep removes that override entirely.
pub fn update_change(changes: &mut Vec<String>, field: &str, value: Option<&str>) {
    changes.retain(|change| !change.starts_with(&format!("{field}=")));
    if let Some(value) = value {
        changes.push(format!("{field}={value}"));
    }
}

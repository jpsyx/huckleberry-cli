//! Stored edit-field values and their finite alternatives.
use crate::cli::{
    Amount, BottleKind, Colour, Consistency, DiaperKind, PottyOutcome, Reaction, Units,
};
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

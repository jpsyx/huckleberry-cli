//! Pump field questions keep current values first and use the shared volume dial.
use anyhow::Result;
use huckleberry_api::models::pump::PumpEntryMode;

use crate::{
    cli::Units,
    edit::{Draft, PumpDraft},
    interactive::options::fields,
};

/// Rebuilds pending amounts in one unit system, including earlier side answers.
/// Reopening the other side after turning units must not reinterpret its number.
pub(super) fn record_volume(
    changes: &mut Vec<String>,
    pump: &PumpDraft,
    field: &str,
    amount: f64,
    units: Units,
) -> Result<()> {
    let mut draft = Draft::Pump(pump.clone());
    draft.set("units", units.as_str())?;
    draft.set(field, &amount.to_string())?;
    let Draft::Pump(updated) = draft else {
        unreachable!("a pump stays a pump")
    };
    replace_volumes(changes, &updated);
    Ok(())
}

/// Removes the selected override, restoring the original field value.
pub(super) fn keep_field(
    changes: &mut Vec<String>,
    original: Option<&Draft>,
    shown: Option<&Draft>,
    field: &str,
) -> Result<()> {
    if field == "units"
        && let (Some(Draft::Pump(original)), Some(Draft::Pump(shown))) = (original, shown)
    {
        record_units(changes, shown, original.units.as_str())?;
    }
    fields::update_change(changes, field, None);
    Ok(())
}

/// Converts current pending measurements before changing their displayed units.
pub(super) fn record_units(changes: &mut Vec<String>, pump: &PumpDraft, units: &str) -> Result<()> {
    let units =
        Units::from_stored(units).ok_or_else(|| anyhow::anyhow!("`{units}` is not ml or oz"))?;
    if units != pump.units {
        for change in changes.iter_mut() {
            let Some((field, value)) = change.split_once('=') else {
                continue;
            };
            if !matches!(
                field.trim().to_lowercase().as_str(),
                "amount" | "left" | "right"
            ) || value.trim().is_empty()
            {
                continue;
            }
            let amount: f64 = value.trim().parse()?;
            *change = format!(
                "{field}={}",
                crate::render::format::convert(amount, pump.units, units)
            );
        }
    }
    fields::update_change(changes, "units", Some(units.as_str()));
    Ok(())
}

/// Writes one complete quantity group, retaining absent values and the chosen mode.
fn replace_volumes(changes: &mut Vec<String>, pump: &PumpDraft) {
    for field in ["mode", "amount", "left", "right", "units"] {
        fields::update_change(changes, field, None);
    }
    fields::update_change(changes, "units", Some(pump.units.as_str()));
    if pump.mode == PumpEntryMode::Total {
        replace_amount(changes, "amount", pump.total_amount());
    } else {
        replace_amount(changes, "left", pump.left_amount);
        replace_amount(changes, "right", pump.right_amount);
    }
}

fn replace_amount(changes: &mut Vec<String>, field: &str, amount: Option<f64>) {
    let value = amount.map_or_else(String::new, |value| value.to_string());
    fields::update_change(changes, field, Some(&value));
}

/// A total's stored halves are not separate measurements to display.
pub(super) fn visible_amount(draft: &Draft, field: &str) -> bool {
    use huckleberry_api::models::pump::PumpEntryMode;
    match draft {
        Draft::Pump(pump) => match pump.mode {
            PumpEntryMode::Total => !matches!(field, "left" | "right"),
            PumpEntryMode::LeftRight => field != "amount",
            PumpEntryMode::Unknown(_) => true,
        },
        _ => true,
    }
}

/// Whether the field's value can be removed.
pub(super) fn can_clear(draft: Option<&Draft>, field: &str) -> bool {
    fields::optional(field)
        || (matches!(draft, Some(Draft::Pump(_))) && matches!(field, "amount" | "left" | "right"))
}

/// Applies pending pump changes using the units shared by explicit amounts.
pub(super) fn shown_draft(mut shown: Draft, changes: &[String]) -> Draft {
    let details = changes
        .iter()
        .filter(|change| {
            change
                .split_once('=')
                .is_some_and(|(field, _)| shown.fields().contains(&field))
        })
        .cloned()
        .collect::<Vec<_>>();
    let _ = shown.apply(&details);
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pump() -> PumpDraft {
        PumpDraft {
            mode: PumpEntryMode::LeftRight,
            left_amount: Some(29.573_529_562_5),
            right_amount: Some(59.147_059_125),
            units: Units::Ml,
            duration_minutes: None,
            notes: None,
        }
    }

    #[test]
    fn changing_units_on_one_side_keeps_the_other_pending_answer() {
        let original = Draft::Pump(pump());
        let mut changes = vec!["notes=keep".into()];
        record_volume(&mut changes, &pump(), "left", 3.0, Units::Oz).unwrap();
        let mut shown = original.clone();
        shown.apply(&changes).unwrap();
        let Draft::Pump(pump) = &shown else {
            panic!("pump")
        };
        assert_eq!(pump.left_amount, Some(3.0));
        assert_eq!(pump.right_amount, Some(2.0));
        record_volume(&mut changes, pump, "right", 60.0, Units::Ml).unwrap();
        let mut saved = original;
        saved.apply(&changes).unwrap();
        let Draft::Pump(saved) = saved else {
            panic!("pump")
        };
        assert_eq!(saved.left_amount, Some(88.720_588_687_5));
        assert_eq!(saved.right_amount, Some(60.0));
        assert_eq!(saved.notes.as_deref(), Some("keep"));
        assert_eq!(saved.units, Units::Ml);
    }

    #[test]
    fn replacing_separate_answers_with_a_total_removes_stale_overrides() {
        let mut changes = vec![
            "left=100".into(),
            "right=200".into(),
            "mode=leftright".into(),
        ];
        record_volume(&mut changes, &pump(), "amount", 2.0, Units::Oz).unwrap();
        let mut saved = Draft::Pump(pump());
        saved.apply(&changes).unwrap();
        let Draft::Pump(saved) = saved else {
            panic!("pump")
        };
        assert_eq!(saved.mode, PumpEntryMode::Total);
        assert_eq!(saved.left_amount, Some(1.0));
        assert_eq!(saved.right_amount, Some(1.0));
    }
}

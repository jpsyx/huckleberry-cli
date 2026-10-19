//! A shared field picker records explicit changes; Keep never normalizes stored data.
#[cfg(test)]
use super::form_pump::keep_field;
use super::form_pump::visible_amount;
use crate::{
    domain::time::Calendar,
    edit::Draft,
    interactive::options::fields,
    prompt::{self, select::MenuItem},
    render::format,
    session::Context,
};
use anyhow::Result;

/// The stored field a sleep's stop time is saved as.
const SLEEP_LENGTH: &str = "duration";

/// Collects the same owned field draft used by the shell's entry editor.
pub async fn collect(
    context: &Context,
    draft: Option<&Draft>,
    started: f64,
    live: bool,
    resumable: bool,
    changes: Vec<String>,
) -> Result<Vec<String>> {
    if !prompt::available() {
        anyhow::bail!("no changes supplied: pass --set at=<TIME> or --set <FIELD=VALUE>");
    }
    let calendar = context.calendar()?;
    let token = if live { super::live::TOKEN } else { "draft" };
    let target = super::retained::EditTarget {
        token: token.into(),
        started,
        draft: draft.cloned(),
        timer: None,
    };
    let time = changes
        .iter()
        .filter_map(|change| change.split_once('='))
        .find(|(field, _)| matches!(*field, "at" | "start"))
        .map(|(_, value)| prompt::time::read_edit_at(context, Some(value), started))
        .transpose()?
        .unwrap_or(started);
    let changes = changes
        .into_iter()
        .filter(|change| !change.starts_with("at=") && !change.starts_with("start="))
        .collect();
    let model = super::retained::EditForm::create_edit(
        calendar.clone(),
        vec![],
        vec![target],
        Some(token.into()),
        vec![],
    )?
    .with_changes(changes, time)
    .with_resumable_sleep(resumable.then_some(token));
    let model = if matches!(draft, Some(Draft::Solids(_))) {
        let (client, cid) = crate::commands::client_and_child(context).await?;
        model.with_foods(&client.custom_foods(&cid, false).await.unwrap_or_default())
    } else {
        model
    };
    let answer =
        prompt::flow::runner::read(model, huckleberry_api::client::now_seconds(), context.theme)?;
    let mut changes = answer.changes;
    if let Some(at) = answer.started {
        fields::update_change(
            &mut changes,
            if live { "start" } else { "at" },
            Some(&calendar.zoned(at).datetime().to_string()),
        );
    }
    Ok(changes)
}

/// Validates and records an answer from a choice or typed field.
pub(super) fn record_value(
    changes: &mut Vec<String>,
    shown: Option<&Draft>,
    field: &str,
    value: &str,
) -> Result<()> {
    if field == "units"
        && let Some(Draft::Pump(pump)) = shown
    {
        return super::form_pump::record_units(changes, pump, value);
    }
    if let Some(mut checked) = shown.cloned() {
        checked.set(field, value)?;
    }
    fields::update_change(changes, field, Some(value));
    Ok(())
}

/// The fields this entry offers in the menu.
pub(super) fn visible_fields(draft: Option<&Draft>, live: bool) -> Vec<&'static str> {
    let time_key = if live { "start" } else { "at" };
    let mut field_names = vec![time_key];
    if let Some(draft) = draft {
        // The units travel with the amount on a dial, so they are not a field
        // of their own to pick. `--set units=oz` still names one.
        field_names.extend(
            draft
                .fields()
                .iter()
                .copied()
                .filter(|field| !folded_into_amount(draft, field) && visible_amount(draft, field)),
        );
    }
    field_names
}

/// The picker's lines: every field with the value the entry would be saved with.
///
/// Showing the value is what the old "keep current" said, except that it says
/// what current is, and a field nobody touched needs no words at all.
pub(super) fn menu_items(
    draft: Option<&Draft>,
    field_names: &[&str],
    changes: &[String],
    started: f64,
    calendar: &Calendar,
) -> Vec<MenuItem> {
    let shown = shown_draft(draft, changes);
    let mut items = vec![MenuItem {
        label: "Done".into(),
        detail: Some("no further changes".into()),
    }];
    for field in field_names {
        let edited = changes
            .iter()
            .any(|change| change.starts_with(&format!("{field}=")));
        let value = shown_value(shown.as_ref(), field, started, calendar);
        items.push(MenuItem {
            label: format!("Edit {}", label_for(draft, field)),
            detail: Some(if edited {
                format!("{value} (changed)")
            } else {
                value
            }),
        });
    }
    items
}

/// What a field is called in the picker: what a parent calls it, not its key.
///
/// A sleep is a stretch of time, so it reads as a start and a stop rather than
/// as the instant it is filed under and the length it lasted.
pub(super) fn label_for<'a>(draft: Option<&Draft>, field: &'a str) -> &'a str {
    let sleeping = matches!(draft, Some(Draft::Sleep(_)));
    match field {
        "at" if sleeping => "start",
        "at" => "time",
        SLEEP_LENGTH if sleeping => "stop",
        other => other,
    }
}

/// The entry as it would be saved: what is stored, with the pending changes on top.
pub(super) fn shown_draft(draft: Option<&Draft>, changes: &[String]) -> Option<Draft> {
    let mut shown = draft.cloned()?;
    if matches!(shown, Draft::Pump(_)) {
        return Some(super::form_pump::shown_draft(shown, changes));
    }
    for change in changes {
        if let Some((field, value)) = change.split_once('=') {
            // A pending value this entry cannot take reads as what is stored.
            let _ = shown.set(field, value);
        }
    }
    Some(shown)
}

/// What one field would be saved as, with a sleep shown by the times it spans.
fn shown_value(draft: Option<&Draft>, field: &str, started: f64, calendar: &Calendar) -> String {
    if matches!(field, "at" | "start") {
        return format::day_time(started, calendar);
    }
    let Some(draft) = draft else {
        return String::new();
    };
    if field == SLEEP_LENGTH
        && let Draft::Sleep(sleep) = draft
    {
        return format::day_time(started + sleep.minutes * 60.0, calendar);
    }
    fields::current(draft, field)
}

/// Records a chosen stop as the length the record stores.
///
/// A stop that lands exactly where the entry already ends is no change at all,
/// so nothing is written for it.
pub(super) fn set_stop(
    changes: &mut Vec<String>,
    draft: Option<&Draft>,
    started: f64,
    stopped: f64,
) {
    let minutes = minutes_of(stopped - started);
    let unchanged =
        stored_minutes(draft).is_some_and(|stored| stored.to_bits() == minutes.to_bits());
    let value = (!unchanged).then(|| minutes_text(minutes));
    fields::update_change(changes, SLEEP_LENGTH, value.as_deref());
}

/// The length a sleep is on the record for, in minutes.
fn stored_minutes(draft: Option<&Draft>) -> Option<f64> {
    match draft {
        Some(Draft::Sleep(sleep)) => Some(minutes_of(sleep.minutes * 60.0)),
        _ => None,
    }
}

/// A span in seconds as the minutes `--set duration=` takes, to the second.
fn minutes_of(seconds: f64) -> f64 {
    (seconds.round() / 60.0 * 10_000.0).round() / 10_000.0
}

/// Those minutes as they are written down, without floating point noise.
fn minutes_text(minutes: f64) -> String {
    let written = format!("{minutes:.4}");
    written
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

/// Whether this field is the sleep length a parent answers as a stop time.
pub(super) fn is_stop(draft: Option<&Draft>, field: &str) -> bool {
    field == SLEEP_LENGTH && matches!(draft, Some(Draft::Sleep(_)))
}

/// Where a sleep currently ends, for the stop question's default.
pub(super) fn stop_of(draft: Option<&Draft>, started: f64) -> f64 {
    match draft {
        Some(Draft::Sleep(sleep)) => started + sleep.minutes * 60.0,
        _ => started,
    }
}

/// Whether a field is answered by another question rather than on its own.
fn folded_into_amount(draft: &Draft, field: &str) -> bool {
    field == "units" && matches!(draft, Draft::Bottle(_) | Draft::Pump(_))
}

/// Compatibility entry for callers collecting a complete history form.
pub async fn fill(
    context: &Context,
    _client: &huckleberry_api::Huckleberry,
    _cid: &str,
    draft: &mut Option<Draft>,
    started: &mut f64,
) -> Result<()> {
    let changes = collect(context, draft.as_ref(), *started, false, false, vec![]).await?;
    for change in &changes {
        let (key, value) = change.split_once('=').expect("collected field change");
        if key == "at" {
            *started = prompt::time::read_edit_at(context, Some(value), *started)?;
        }
    }
    if let Some(draft) = draft {
        replay_details(draft, &changes)?;
    }
    Ok(())
}

/// Applies collected detail answers while keeping time on its separate path.
fn replay_details(draft: &mut Draft, changes: &[String]) -> Result<()> {
    let details = changes
        .iter()
        .filter(|change| !change.starts_with("at="))
        .cloned()
        .collect::<Vec<_>>();
    draft.apply(&details)
}

#[cfg(test)]
mod picker {
    use super::*;
    use crate::domain::time::Calendar;
    use crate::edit::{SleepDraft, SolidsDraft};

    /// Jan 1, 2025 at 07:00 UTC, the start every sleep here is measured from.
    const STARTED: f64 = 1_735_714_800.0;

    fn sleep() -> Draft {
        Draft::Sleep(SleepDraft {
            minutes: 90.0,
            notes: None,
        })
    }

    fn lines(draft: &Draft, fields: &[&str], changes: &[String], started: f64) -> Vec<String> {
        let calendar = Calendar::new("UTC").expect("a calendar");
        menu_items(Some(draft), fields, changes, started, &calendar)
            .into_iter()
            .map(|item| match item.detail {
                Some(detail) => format!("{}: {detail}", item.label),
                None => item.label,
            })
            .collect()
    }

    #[test]
    fn a_sleep_is_edited_as_a_start_and_a_stop_showing_both_times() {
        let shown = lines(&sleep(), &["at", "duration", "notes"], &[], STARTED);
        assert_eq!(shown[1], "Edit start: Jan 01, 7:00 am UTC");
        assert_eq!(shown[2], "Edit stop: Jan 01, 8:30 am UTC");
        assert_eq!(shown[3], "Edit notes: not set");
        assert!(!shown.iter().any(|line| line.contains("keep current")));
    }

    #[test]
    fn a_pending_change_shows_the_value_it_would_be_saved_with() {
        let changes = vec!["duration=150".to_owned(), "notes=slept well".to_owned()];
        let shown = lines(&sleep(), &["at", "duration", "notes"], &changes, STARTED);
        assert_eq!(shown[2], "Edit stop: Jan 01, 9:30 am UTC (changed)");
        assert_eq!(shown[3], "Edit notes: slept well (changed)");
    }

    #[test]
    fn every_other_entry_keeps_its_own_field_names_and_shows_what_is_stored() {
        let meal = Draft::Solids(SolidsDraft {
            foods: vec!["pear".into(), "oats".into()],
            amount: "some".into(),
            reaction: None,
            notes: None,
        });
        let shown = lines(&meal, &["at", "foods", "reaction"], &[], STARTED);
        assert_eq!(shown[1], "Edit time: Jan 01, 7:00 am UTC");
        assert_eq!(shown[2], "Edit foods: pear, oats");
        assert_eq!(shown[3], "Edit reaction: not set");
    }

    #[test]
    fn a_stop_is_stored_as_the_minutes_between_it_and_the_start() {
        let mut changes = vec![];
        set_stop(&mut changes, Some(&sleep()), STARTED, STARTED + 9000.0);
        assert_eq!(changes, ["duration=150"]);
        // A stop exactly where the entry already ends is not a change.
        set_stop(&mut changes, Some(&sleep()), STARTED, STARTED + 5400.0);
        assert!(changes.is_empty());
    }
}

#[cfg(test)]
mod pump_form {
    use super::*;
    use crate::edit::PumpDraft;
    use crate::prompt::host;
    use crate::theme::Theme;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use huckleberry_api::models::pump::PumpEntryMode;

    fn pump() -> Draft {
        Draft::Pump(PumpDraft {
            mode: PumpEntryMode::Total,
            left_amount: Some(30.12345),
            right_amount: Some(30.12345),
            units: crate::cli::Units::Ml,
            duration_minutes: Some(15.0),
            notes: None,
        })
    }

    fn answer(
        field: &'static str,
        expected: &'static str,
        keys: Vec<KeyCode>,
    ) -> Result<Option<String>> {
        use crate::prompt::flow::FormState;
        let model = super::super::retained::EditForm::create_edit(
            Calendar::new("UTC")?,
            vec![],
            vec![super::super::retained::EditTarget {
                token: "pump/entry".into(),
                started: 1000.0,
                draft: Some(pump()),
                timer: None,
            }],
            Some("pump/entry".into()),
            vec![],
        )?;
        let mut form = FormState::create_form(model, 10000.0)?;
        let index = visible_fields(Some(&pump()), false)
            .iter()
            .position(|name| *name == field)
            .unwrap()
            + 1;
        let press = |form: &mut FormState<_>, code| {
            form.apply(
                host::Input::Key(KeyEvent::new(code, KeyModifiers::NONE)),
                10000.0,
                (80, 24),
            )
        };
        for _ in 0..index {
            press(&mut form, KeyCode::Down);
        }
        press(&mut form, KeyCode::Enter);
        assert!(
            form.render(10000.0, (80, 24), Theme::dark(false))
                .join("\n")
                .contains(expected)
        );
        for code in keys {
            press(&mut form, code);
        }
        Ok(form
            .complete()?
            .changes
            .iter()
            .filter_map(|change| change.split_once('='))
            .find(|(name, _)| *name == field)
            .map(|(_, value)| value.to_owned()))
    }

    #[test]
    fn pump_mode_offers_pump_choices_and_keeps_current_on_enter() {
        let result = answer(
            "mode",
            "Left and right",
            vec![KeyCode::Char('3'), KeyCode::Enter],
        )
        .unwrap();
        assert!(matches!(result, Some(value) if value == "leftright"));
    }

    #[test]
    fn pump_volume_can_clear_without_typing_and_keep_exact_quantities() {
        let kept = answer("amount", "Clear value", vec![KeyCode::Enter]).unwrap();
        assert!(kept.is_none());
        let cleared = answer(
            "amount",
            "Clear value",
            vec![KeyCode::Char('3'), KeyCode::Enter],
        )
        .unwrap();
        assert!(matches!(cleared, Some(value) if value.is_empty()));
    }

    #[test]
    fn pump_duration_has_presets_and_a_clear_choice() {
        let result = answer(
            "duration",
            "20 minutes",
            vec![KeyCode::Char('7'), KeyCode::Enter],
        )
        .unwrap();
        assert!(matches!(result, Some(value) if value == "20"));
    }
    #[test]
    fn total_pump_fields_hide_artificial_side_amounts() {
        let fields = visible_fields(Some(&pump()), false);
        assert!(fields.contains(&"amount"));
        assert!(!fields.contains(&"left"));
        assert!(!fields.contains(&"right"));
    }

    #[test]
    fn pending_mode_changes_immediately_replace_visible_amount_fields() {
        let original = pump();
        let shown = shown_draft(Some(&original), &["mode=leftright".into()]);
        let fields = visible_fields(shown.as_ref(), false);
        assert!(!fields.contains(&"amount"));
        assert!(fields.contains(&"left"));
        assert!(fields.contains(&"right"));
        assert!(original.fields().contains(&"amount"));
        assert!(original.fields().contains(&"left"));
        let shown = shown_draft(shown.as_ref(), &["mode=total".into()]);
        let fields = visible_fields(shown.as_ref(), false);
        assert!(fields.contains(&"amount"));
        assert!(!fields.contains(&"left"));
        assert!(!fields.contains(&"right"));
    }

    #[test]
    fn unknown_pump_modes_keep_every_amount_available_for_inspection() {
        let Draft::Pump(mut pump) = pump() else {
            panic!("pump")
        };
        pump.mode = PumpEntryMode::Unknown("future".into());
        let draft = Draft::Pump(pump);
        let fields = visible_fields(Some(&draft), false);
        assert!(fields.contains(&"amount"));
        assert!(fields.contains(&"left"));
        assert!(fields.contains(&"right"));
    }

    /// Runs the same pending-value path used by standalone Edit answers.
    fn typed_changes(original: &Draft, answers: &[(&str, &str)]) -> Draft {
        let mut changes = Vec::new();
        for (field, value) in answers {
            let shown = shown_draft(Some(original), &changes);
            record_value(&mut changes, shown.as_ref(), field, value).unwrap();
        }
        let mut saved = original.clone();
        replay_details(&mut saved, &changes).unwrap();
        saved
    }

    fn separate_pump() -> Draft {
        let Draft::Pump(mut pump) = pump() else {
            panic!("pump")
        };
        pump.mode = PumpEntryMode::LeftRight;
        pump.left_amount = Some(30.0);
        pump.right_amount = Some(70.0);
        Draft::Pump(pump)
    }

    #[test]
    fn typed_unit_changes_convert_the_pending_side_answer() {
        let Draft::Pump(saved) =
            typed_changes(&separate_pump(), &[("left", "60"), ("units", "oz")])
        else {
            panic!("pump")
        };
        assert!((saved.left_amount.unwrap() - 2.028_841_362_110_58).abs() < 1e-12);
        assert!((saved.right_amount.unwrap() - 2.366_981_589_129_01).abs() < 1e-12);
        assert_eq!(saved.units, crate::cli::Units::Oz);
    }

    #[test]
    fn typed_unit_changes_preserve_a_pending_total_mode_switch() {
        let Draft::Pump(saved) = typed_changes(
            &separate_pump(),
            &[("left", "60"), ("mode", "total"), ("units", "oz")],
        ) else {
            panic!("pump")
        };
        assert_eq!(saved.mode, PumpEntryMode::Total);
        assert!((saved.left_amount.unwrap() - 2.197_911_475_619_79).abs() < 1e-12);
        assert!((saved.right_amount.unwrap() - 2.197_911_475_619_79).abs() < 1e-12);
    }

    #[test]
    fn typed_unit_changes_keep_cleared_sides_absent() {
        let Draft::Pump(saved) = typed_changes(
            &separate_pump(),
            &[("left", "60"), ("right", ""), ("units", "oz")],
        ) else {
            panic!("pump")
        };
        assert_eq!(saved.right_amount, None);
        assert!((saved.left_amount.unwrap() - 2.028_841_362_110_58).abs() < 1e-12);
    }

    #[test]
    fn keeping_original_units_converts_pending_quantities_back() {
        let original = separate_pump();
        let mut changes = vec!["units=oz".into(), "left=2".into(), "right=3".into()];
        let shown = shown_draft(Some(&original), &changes);
        keep_field(&mut changes, Some(&original), shown.as_ref(), "units").unwrap();
        assert!(!changes.iter().any(|change| change.starts_with("units=")));
        let mut saved = original;
        saved.apply(&changes).unwrap();
        let Draft::Pump(saved) = saved else {
            panic!("pump")
        };
        assert_eq!(saved.units, crate::cli::Units::Ml);
        assert_eq!(saved.left_amount, Some(59.147_059_125));
        assert_eq!(saved.right_amount, Some(88.720_588_687_5));
    }
    #[test]
    fn interactive_units_keep_unknown_modes_and_do_not_add_amount_overrides() {
        let Draft::Pump(mut original) = separate_pump() else {
            panic!("pump")
        };
        original.mode = PumpEntryMode::Unknown("future".into());
        let original = Draft::Pump(original);
        let mut changes = vec!["notes=changed".into()];
        record_value(&mut changes, Some(&original), "units", "oz").unwrap();
        assert!(
            changes
                .iter()
                .all(|change| change.starts_with("notes=") || change.starts_with("units="))
        );
        let mut saved = original;
        saved.apply(&changes).unwrap();
        let Draft::Pump(saved) = saved else {
            panic!("pump")
        };
        assert_eq!(saved.mode, PumpEntryMode::Unknown("future".into()));
    }

    #[test]
    fn changing_pending_units_only_converts_existing_amount_overrides() {
        let original = separate_pump();
        let mut changes = vec!["left=60".into(), "mode=total".into()];
        let shown = shown_draft(Some(&original), &changes);
        record_value(&mut changes, shown.as_ref(), "units", "oz").unwrap();
        assert!(changes.iter().any(|change| change.starts_with("left=")));
        assert!(changes.iter().any(|change| change == "mode=total"));
        assert!(
            !changes
                .iter()
                .any(|change| change.starts_with("right=") || change.starts_with("amount="))
        );
    }
}

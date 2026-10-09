//! A shared field picker records explicit changes; Keep never normalizes stored data.
use super::form_pump::{can_clear, keep_field, uses_pump_prompt, visible_amount};
use crate::{
    domain::time::Calendar,
    edit::Draft,
    interactive::options::fields,
    prompt::{
        self, Question,
        select::{self, MenuItem},
    },
    render::format,
    session::Context,
};
use anyhow::Result;

/// The stored field a sleep's stop time is saved as.
const SLEEP_LENGTH: &str = "duration";

/// What one answered field contributes to the pending changes.
pub(super) enum Answer {
    /// A new stored value for the field.
    Value(String),
    /// The instant the entry now starts at.
    Start(f64),
    /// The instant a sleep now ends at.
    Stop(f64),
    /// One pump volume answer, retaining the side it belongs to.
    PumpVolume {
        field: String,
        amount: f64,
        units: crate::cli::Units,
    },
    /// An amount and the units it is in, which are one answer.
    Volume {
        /// The amount, as it is stored.
        amount: String,
        /// The units it is in.
        units: String,
    },
}

/// Collects only deliberate changes, starting on Done and keeping every other field.
pub async fn collect(
    context: &Context,
    draft: Option<&Draft>,
    started: f64,
    live: bool,
    mut changes: Vec<String>,
) -> Result<Vec<String>> {
    if !prompt::available() {
        anyhow::bail!("no changes supplied: pass --set at=<TIME> or --set <FIELD=VALUE>");
    }
    let calendar = context.calendar()?;
    let original = started;
    let mut started = started;
    let mut stopped = None;
    loop {
        let shown = shown_draft(draft, &changes);
        let field_names = visible_fields(shown.as_ref(), live);
        let items = menu_items(draft, &field_names, &changes, started, &calendar);
        let index = choose(context, "Fields to change", &items)?;
        if index == 0 {
            return Ok(changes);
        }
        let field = field_names[index - 1];
        match read(context, field, draft, shown.as_ref(), started).await? {
            None => keep_field(&mut changes, draft, shown.as_ref(), field)?,
            Some(Answer::Value(value)) => {
                record_value(&mut changes, shown.as_ref(), field, &value)?;
            }
            Some(Answer::Start(instant)) => {
                started = instant;
                let moved = (instant.to_bits() != original.to_bits())
                    .then(|| calendar.zoned(instant).datetime().to_string());
                fields::update_change(&mut changes, field, moved.as_deref());
                keep_stop(context, &mut changes, &mut stopped, draft, started);
            }
            Some(Answer::Stop(instant)) => {
                stopped = Some(instant);
                set_stop(&mut changes, draft, started, instant);
            }
            Some(Answer::PumpVolume {
                field,
                amount,
                units,
            }) => {
                if let Some(Draft::Pump(pump)) = &shown {
                    super::form_pump::record_volume(&mut changes, pump, &field, amount, units)?;
                }
            }
            Some(Answer::Volume { amount, units }) => {
                if let Some(mut checked) = draft.cloned() {
                    checked.set("amount", &amount)?;
                    checked.set("units", &units)?;
                }
                fields::update_change(&mut changes, "amount", Some(&amount));
                fields::update_change(&mut changes, "units", Some(&units));
            }
        }
    }
}

/// Validates and records an answer from a choice or typed field.
fn record_value(
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
fn visible_fields(draft: Option<&Draft>, live: bool) -> Vec<&'static str> {
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
fn menu_items(
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
fn label_for<'a>(draft: Option<&Draft>, field: &'a str) -> &'a str {
    let sleeping = matches!(draft, Some(Draft::Sleep(_)));
    match field {
        "at" if sleeping => "start",
        "at" => "time",
        SLEEP_LENGTH if sleeping => "stop",
        other => other,
    }
}

/// The entry as it would be saved: what is stored, with the pending changes on top.
fn shown_draft(draft: Option<&Draft>, changes: &[String]) -> Option<Draft> {
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
fn set_stop(changes: &mut Vec<String>, draft: Option<&Draft>, started: f64, stopped: f64) {
    let minutes = minutes_of(stopped - started);
    let unchanged =
        stored_minutes(draft).is_some_and(|stored| stored.to_bits() == minutes.to_bits());
    let value = (!unchanged).then(|| minutes_text(minutes));
    fields::update_change(changes, SLEEP_LENGTH, value.as_deref());
}

/// A stop a parent already chose stays where they put it when the start moves.
fn keep_stop(
    context: &Context,
    changes: &mut Vec<String>,
    stopped: &mut Option<f64>,
    draft: Option<&Draft>,
    started: f64,
) {
    let Some(stop) = *stopped else {
        return;
    };
    if stop > started {
        set_stop(changes, draft, started, stop);
    } else {
        *stopped = None;
        fields::update_change(changes, SLEEP_LENGTH, None);
        context.warn("That start is after the stop you chose: choose Edit stop again.");
    }
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
fn is_stop(draft: Option<&Draft>, field: &str) -> bool {
    field == SLEEP_LENGTH && matches!(draft, Some(Draft::Sleep(_)))
}

/// Where a sleep currently ends, for the stop question's default.
fn stop_of(draft: Option<&Draft>, started: f64) -> f64 {
    match draft {
        Some(Draft::Sleep(sleep)) => started + sleep.minutes * 60.0,
        _ => started,
    }
}

/// Whether a field is answered by another question rather than on its own.
fn folded_into_amount(draft: &Draft, field: &str) -> bool {
    field == "units" && matches!(draft, Draft::Bottle(_) | Draft::Pump(_)) && prompt::host::hosted()
}

/// Asks for one field. Times are asked outright: their default already keeps.
async fn read(
    context: &Context,
    field: &str,
    draft: Option<&Draft>,
    shown: Option<&Draft>,
    started: f64,
) -> Result<Option<Answer>> {
    if matches!(field, "at" | "start") {
        return Ok(Some(Answer::Start(if field == "at" {
            prompt::time::read_edit_at(context, None, started)?
        } else {
            prompt::time::read_edit_start(context, None, Some(started))?.unwrap_or(started)
        })));
    }
    if let Some(Draft::Pump(pump)) = shown
        && uses_pump_prompt(field, prompt::host::hosted())
    {
        return super::form_pump::read(context, pump, field);
    }
    // A volume is one question. The units column travels with the amount, so
    // there is no second field to pick and no second answer to keep in step.
    if field == "amount"
        && prompt::host::hosted()
        && let Some(Draft::Bottle(bottle)) = shown
    {
        let (units, amount) = prompt::dial::volume::ask(
            "How much?",
            bottle.units,
            Some(bottle.amount),
            context.theme,
        )?;
        return Ok(Some(Answer::Volume {
            amount: format::amount_in(amount, units),
            units: units.as_str().to_owned(),
        }));
    }
    if is_stop(draft, field) {
        let stopped =
            prompt::time::read_edit_stop(context, None, started, stop_of(shown, started))?;
        return Ok(Some(Answer::Stop(stopped)));
    }
    let mut labels = vec!["Keep current value".into()];
    let choices = fields::choices(field);
    labels.extend(choices.iter().map(|(_, label)| label.clone()));
    if choices.is_empty() {
        labels.push("Enter a new value".into());
    }
    if can_clear(draft, field) {
        labels.push("Clear value".into());
    }
    let index = choose(context, label_for(draft, field), &items(&labels))?;
    if index == 0 {
        return Ok(None);
    }
    if labels[index] == "Clear value" {
        return Ok(Some(Answer::Value(String::new())));
    }
    if let Some((value, _)) = choices.get(index - 1) {
        return Ok(Some(Answer::Value(value.clone())));
    }
    if field == "foods" {
        return food_value(context, draft)
            .await
            .map(Answer::Value)
            .map(Some);
    }
    let question = Question::new("field value", "New value?", "--set <KEY=VALUE>");
    Ok(Some(Answer::Value(prompt::ask(&question, context.theme)?)))
}

async fn food_value(context: &Context, draft: Option<&Draft>) -> Result<String> {
    let current = match draft {
        Some(Draft::Solids(meal)) => meal.foods.clone(),
        _ => vec![],
    };
    let (client, cid) = crate::commands::client_and_child(context).await?;
    Ok(
        crate::commands::feed::solids::ask_for_foods(context, &client, &cid, &current)
            .await?
            .join(", "),
    )
}

/// Plain menu entries, for the lists that are only labels.
fn items(labels: &[String]) -> Vec<MenuItem> {
    labels
        .iter()
        .map(|label| MenuItem {
            label: label.clone(),
            detail: None,
        })
        .collect()
}

fn choose(context: &Context, title: &str, items: &[MenuItem]) -> Result<usize> {
    select::choose(title, items, 0, context.theme)
}

/// Compatibility entry for callers collecting a complete history form.
pub async fn fill(
    context: &Context,
    _client: &huckleberry_api::Huckleberry,
    _cid: &str,
    draft: &mut Option<Draft>,
    started: &mut f64,
) -> Result<()> {
    let changes = collect(context, draft.as_ref(), *started, false, vec![]).await?;
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

    fn context() -> Context {
        Context {
            config: crate::config::Config::default(),
            config_path: "unused.toml".into(),
            credentials_path: "unused.json".into(),
            theme: Theme::dark(false),
            verbose: false,
            child_override: None,
            offline: None,
        }
    }

    /// Exercises the field prompt through the host, without terminal or network access.
    fn answer(
        field: &'static str,
        expected: &'static str,
        keys: Vec<KeyCode>,
    ) -> Result<Option<Answer>> {
        let _one = host::one_at_a_time();
        let channel = host::install();
        let worker = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            let draft = pump();
            runtime.block_on(read(&context(), field, Some(&draft), Some(&draft), 1000.0))
        });
        let mut keys = keys.into_iter();
        let mut saw_expected = false;
        while let Ok((request, reply)) = channel
            .requests
            .recv_timeout(std::time::Duration::from_millis(100))
        {
            match request {
                host::Request::Frame(lines) => {
                    saw_expected |= lines.iter().any(|line| line.contains(expected));
                    let key = keys.next().unwrap_or(KeyCode::Esc);
                    reply
                        .send(host::Reply::Input(host::Input::Key(KeyEvent::new(
                            key,
                            KeyModifiers::NONE,
                        ))))
                        .unwrap();
                }
                host::Request::Step(_) => {
                    reply
                        .send(host::Reply::Stepped { interrupted: true })
                        .unwrap();
                }
                host::Request::Show(_) => {
                    reply.send(host::Reply::Shown).unwrap();
                }
            }
        }
        host::remove();
        let result = worker.join().unwrap();
        assert!(saw_expected, "the {field} form did not show {expected}");
        result
    }

    #[test]
    fn pump_mode_offers_pump_choices_and_keeps_current_on_enter() {
        let result = answer(
            "mode",
            "Left and right",
            vec![KeyCode::Char('3'), KeyCode::Enter],
        )
        .unwrap();
        assert!(matches!(result, Some(Answer::Value(value)) if value == "leftright"));
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
        assert!(matches!(cleared, Some(Answer::Value(value)) if value.is_empty()));
    }

    #[test]
    fn pump_duration_has_presets_and_a_clear_choice() {
        let result = answer(
            "duration",
            "20 minutes",
            vec![KeyCode::Char('7'), KeyCode::Enter],
        )
        .unwrap();
        assert!(matches!(result, Some(Answer::Value(value)) if value == "20"));
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

    #[test]
    fn standalone_pump_volumes_use_text_with_clear_while_shell_uses_the_dial() {
        for field in ["amount", "left", "right"] {
            assert!(!uses_pump_prompt(field, false));
            assert!(uses_pump_prompt(field, true));
            assert!(can_clear(Some(&pump()), field));
        }
        for field in ["mode", "duration"] {
            assert!(uses_pump_prompt(field, false));
        }
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

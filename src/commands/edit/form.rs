//! A shared field picker records explicit changes; Keep never normalizes stored data.
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
enum Answer {
    /// A new stored value for the field.
    Value(String),
    /// The instant the entry now starts at.
    Start(f64),
    /// The instant a sleep now ends at.
    Stop(f64),
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
    let time_key = if live { "start" } else { "at" };
    let mut field_names = vec![time_key];
    if let Some(draft) = draft {
        field_names.extend(draft.fields());
    }
    let original = started;
    let mut started = started;
    let mut stopped = None;
    loop {
        let shown = shown_draft(draft, &changes);
        let items = menu_items(draft, &field_names, &changes, started, &calendar);
        let index = choose(context, "Fields to change", &items)?;
        if index == 0 {
            return Ok(changes);
        }
        let field = field_names[index - 1];
        match read(context, field, draft, shown.as_ref(), started).await? {
            None => fields::update_change(&mut changes, field, None),
            Some(Answer::Value(value)) => {
                if let Some(mut checked) = draft.cloned() {
                    checked.set(field, &value)?;
                }
                fields::update_change(&mut changes, field, Some(&value));
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
        }
    }
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
        return format::date_time(started, calendar);
    }
    let Some(draft) = draft else {
        return String::new();
    };
    if field == SLEEP_LENGTH
        && let Draft::Sleep(sleep) = draft
    {
        return format::date_time(started + sleep.minutes * 60.0, calendar);
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
    if fields::optional(field) {
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
    for change in changes {
        let (key, value) = change.split_once('=').expect("collected field change");
        if key == "at" {
            *started = prompt::time::read_edit_at(context, Some(value), *started)?;
        } else if let Some(draft) = draft {
            draft.set(key, value)?;
        }
    }
    Ok(())
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
        assert_eq!(shown[1], "Edit start: Jan 01, 2025, 7:00 am UTC");
        assert_eq!(shown[2], "Edit stop: Jan 01, 2025, 8:30 am UTC");
        assert_eq!(shown[3], "Edit notes: not set");
        assert!(!shown.iter().any(|line| line.contains("keep current")));
    }

    #[test]
    fn a_pending_change_shows_the_value_it_would_be_saved_with() {
        let changes = vec!["duration=150".to_owned(), "notes=slept well".to_owned()];
        let shown = lines(&sleep(), &["at", "duration", "notes"], &changes, STARTED);
        assert_eq!(shown[2], "Edit stop: Jan 01, 2025, 9:30 am UTC (changed)");
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
        assert_eq!(shown[1], "Edit time: Jan 01, 2025, 7:00 am UTC");
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

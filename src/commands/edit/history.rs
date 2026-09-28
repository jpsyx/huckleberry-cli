//! History edits combine optional detail forms with a time shared by every tracker.

use anyhow::{Context as _, Result, bail};
use huckleberry_api::models::health::HealthEntry;
use huckleberry_api::{Huckleberry, Located, RowRef, Window, client::now_seconds};

use crate::cli::Units;
use crate::domain::{
    log::{Entry, Kind},
    types::Dataset,
};
use crate::edit::{self, Draft};
use crate::render::format;
use crate::session::Context;

/// Adds growth and the other health rows that are absent from the merged activity log.
pub async fn add_health(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    days: u32,
    entries: &mut Vec<Entry>,
) -> Result<()> {
    let now = now_seconds() as i64;
    match client
        .health_entries(cid, Window::new(now - i64::from(days) * 86_400, now))
        .await
    {
        Ok(rows) => entries.extend(rows.into_iter().map(health_entry)),
        Err(error) => context.warn(&format!(
            "Health history: {}",
            crate::dataset::describe(&error)
        )),
    }
    entries.sort_by(|left, right| right.start.total_cmp(&left.start));
    Ok(())
}

/// A health row in the same picker as other entries; its time is editable.
fn health_entry(located: Located<HealthEntry>) -> Entry {
    let title = match &located.row {
        HealthEntry::Growth(_) => "Growth",
        HealthEntry::Medication(_) => "Medication",
        HealthEntry::Temperature(_) => "Temperature",
    };
    let description = health_description(&located.row);
    Entry {
        id: edit::token_for(&located.at),
        at: Some(located.at),
        kind: Kind::Health,
        start: located.row.start(),
        title: title.into(),
        description,
        notes: None,
    }
}

/// Shows the measurements that distinguish entries taken near each other.
fn health_description(row: &HealthEntry) -> String {
    let json = serde_json::to_value(row).unwrap_or_default();
    [
        ("weight", "weightUnits"),
        ("height", "heightUnits"),
        ("head", "headUnits"),
        ("amount", "units"),
    ]
    .into_iter()
    .filter_map(|(field, units)| {
        let value = json.get(field)?;
        Some(format!(
            "{field}: {value} {}",
            json.get(units)
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
        ))
    })
    .collect::<Vec<_>>()
    .join(" · ")
}

/// Edits the selected row, returning a receipt with its possibly changed reference.
pub async fn run(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    dataset: &Dataset,
    entries: &[Entry],
    token: &str,
    changes: &[String],
) -> Result<()> {
    let at = edit::parse_token(token)?;
    let current = started_at(client, cid, entries, &at).await?;
    let mut started = current;
    let mut draft = edit::draft_for(dataset, &at, Units::from_setting(&context.config.units));
    let before = draft.clone();
    fill(context, client, cid, changes, &mut draft, &mut started).await?;
    let explicit_details = changes.iter().any(|change| !change.starts_with("at="));
    if draft == before && !explicit_details && started.to_bits() == current.to_bits() {
        context.report("The entry is unchanged.");
        return Ok(());
    }
    if (draft != before || explicit_details)
        && let Some(details) = &draft
        && let Some(before) = &before
    {
        let original = super::preserve::Original::from_entry(dataset, &at, before, changes);
        super::save::run(client, cid, &at, details, &original).await?;
    }
    let saved =
        if started.to_bits() == current.to_bits() {
            at
        } else {
            client.update_history_time(cid, &at, started).await.context(
            "changing the entry time (any preceding detail changes have already been saved)",
        )?
        };
    super::super::persist_session(context, client).await?;
    receipt(
        context,
        &saved,
        current,
        started,
        before.as_ref(),
        draft.as_ref(),
    )
}

/// Resolves and validates all answers before any detail or time writes.
async fn fill(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    changes: &[String],
    draft: &mut Option<Draft>,
    started: &mut f64,
) -> Result<()> {
    if changes.is_empty() {
        super::form::fill(context, client, cid, draft, started).await
    } else {
        let (time, details) = split_changes(changes)?;
        if let Some(time) = time {
            *started = crate::prompt::time::read_edit_at(context, Some(time), *started)?;
        }
        apply_details(draft, &details)
    }
}

/// Finds the stored instant even when an explicit selector is outside the picker window.
async fn started_at(
    client: &Huckleberry,
    cid: &str,
    entries: &[Entry],
    at: &RowRef,
) -> Result<f64> {
    if let Some(entry) = entries.iter().find(|entry| entry.at.as_ref() == Some(at)) {
        return Ok(entry.start);
    }
    client
        .located_rows(&at.tracker, cid, "finding the entry to edit")
        .await?
        .into_iter()
        .find(|(found, _)| found == at)
        .and_then(|(_, row)| row.get("start").and_then(serde_json::Value::as_f64))
        .with_context(|| format!("no timed entry `{}` on the record", edit::token_for(at)))
}

/// Time belongs to every entry, including trackers without a detail form.
fn split_changes(changes: &[String]) -> Result<(Option<&str>, Vec<String>)> {
    let mut time = None;
    let mut details = Vec::new();
    for change in changes {
        let (key, value) = change
            .split_once('=')
            .with_context(|| format!("`{change}` is not key=value"))?;
        if matches!(key.trim().to_lowercase().as_str(), "at" | "start") {
            if time.is_some() || value.trim().is_empty() {
                bail!("supply one nonempty --set at=<TIME>");
            }
            time = Some(value.trim());
        } else {
            details.push(change.clone());
        }
    }
    Ok((time, details))
}

/// Rejects unsupported fields before any part of the edit is saved.
fn apply_details(draft: &mut Option<Draft>, changes: &[String]) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    draft
        .as_mut()
        .context("this entry supports only --set at=<TIME>")?
        .apply(changes)
}

/// Makes a time-only change visible and gives scripts the new selector.
fn receipt(
    context: &Context,
    at: &RowRef,
    previous: f64,
    started: f64,
    before: Option<&Draft>,
    after: Option<&Draft>,
) -> Result<()> {
    let calendar = context.calendar()?;
    let mut fields = vec![
        ("Previous time", format::date_time(previous, &calendar)),
        ("When", format::date_time(started, &calendar)),
    ];
    if let (Some(before), Some(after)) = (before, after) {
        fields.push(("Before", edit::summary(before)));
        fields.push(("After", edit::summary(after)));
    }
    fields.push(("Entry ID", edit::token_for(at)));
    context.receipt(
        "📝 Entry updated",
        &fields,
        &[format!("entry\t{}", edit::token_for(at))],
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn time_changes_are_separated_and_duplicates_are_rejected() {
        let changes = vec!["amount=90".into(), "at=8am".into()];
        let (time, details) = split_changes(&changes).unwrap();
        assert_eq!(time, Some("8am"));
        assert_eq!(details, vec!["amount=90"]);
        for invalid in [
            vec!["at=".into()],
            vec!["at=8am".into(), "start=9am".into()],
        ] {
            assert!(split_changes(&invalid).is_err());
        }
        assert!(apply_details(&mut None, &[]).is_ok());
        assert!(apply_details(&mut None, &["amount=90".into()]).is_err());
    }

    #[test]
    fn growth_history_can_be_selected_for_a_time_correction() {
        let row: HealthEntry = serde_json::from_value(serde_json::json!({
            "mode": "growth", "start": 1000.0, "lastUpdated": 1000.0,
            "offset": 0, "weight": 3.6, "weightUnits": "kg",
        }))
        .unwrap();
        let entry = health_entry(Located::new(RowRef::loose("health", "1000000-abc"), row));
        assert_eq!(entry.title, "Growth");
        assert_eq!(entry.start.to_bits(), 1000.0_f64.to_bits());
        assert_eq!(entry.at.unwrap().tracker, "health");
        assert!(entry.description.contains("3.6 kg"));
    }
}

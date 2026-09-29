//! `edit`: changing an entry that is already on the record.
//!
//! The two audiences meet here the way they do everywhere else in this tool.
//! A person runs `edit`, picks an entry, then chooses fields to change.
//! Every unselected field keeps its exact stored value. A
//! script runs `edit --list` to find the entry it means and
//! `edit --id <ENTRY> --set key=value` to change it, and never meets a prompt.
//!
//! This module owns the terminal, the keyboard and the network. What an edit
//! *is* lives in [`crate::edit`], the list and the cursor in
//! [`crate::listing`], and the questions in [`form`].
//!
//! History time corrections preserve the row's other data and return its updated
//! reference. Live sleeps use a separate timer operation.

pub mod form;
mod history;
mod live;
pub mod pick;
pub mod save;
mod save_foods;
mod save_values;

use anyhow::{Result, bail};
#[cfg(test)]
use huckleberry_api::RowRef;
use huckleberry_api::client::now_seconds;

use crate::cli::{EditOptions, Units};
use crate::domain::log::Entry;
use crate::domain::{Calendar, log};
use crate::edit;
use crate::listing::Listing;
use crate::render::format;
use crate::session::Context;

/// Runs the command.
pub async fn run(context: &Context, options: &EditOptions) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    if options.id.as_deref() == Some(live::TOKEN) && !options.list {
        return live::run(context, &client, &cid, &options.set).await;
    }
    let window = context.days(options.days);
    context.narrate(&format!("Reading {window} days from Huckleberry..."));
    let dataset = crate::dataset::pull(
        &client,
        &cid,
        None,
        window,
        &context.config.timezone,
        now_seconds(),
    )
    .await?;
    super::persist_session(context, &client).await?;
    let calendar = Calendar::new(&dataset.timezone)?;
    let mut entries = log::build(&dataset, |amount| {
        format::volume(amount, Units::from_setting(&context.config.units))
    });
    history::add_health(context, &client, &cid, window, &mut entries).await?;
    let mut shown = entries.clone();
    shown.truncate(options.limit);

    if let Some(ongoing) = live::entry(&dataset.live, now_seconds()) {
        shown.insert(0, ongoing);
    }

    if options.list {
        list(context, &shown, &calendar);
        return Ok(());
    }

    let picked = match options.id.as_deref() {
        Some(token) => Some(token.to_owned()),
        None => choose(context, &shown, &calendar)?,
    };
    // Leaving the list without picking is not a failure: nothing was asked
    // for, so nothing happened and nothing is reported as having gone wrong.
    let Some(token) = picked else {
        context.report("Nothing changed.");
        return Ok(());
    };

    if token == live::TOKEN {
        return live::run(context, &client, &cid, &options.set).await;
    }
    history::run(
        context,
        &client,
        &cid,
        &dataset,
        &entries,
        &token,
        &options.set,
    )
    .await
}

/// Prints the entries, one `key<TAB>value` row each, for a script to read.
///
/// On stdout, because this is the answer the command was asked for. An entry
/// this tool cannot change says so in its own column rather than being left
/// out, so a script does not have to guess why its id is missing.
fn list(context: &Context, entries: &[Entry], calendar: &Calendar) {
    let mut machine = Vec::new();
    let mut rows = Vec::new();
    for entry in entries {
        let Some(token) = entry_token(entry) else {
            continue;
        };
        machine.push(format!(
            "{}\t{}\t{} {}\t{}\t{}",
            token,
            entry.kind.as_str(),
            format::day_short(calendar.day_of(entry.start)),
            format::clock(entry.start, calendar),
            if editable(entry) {
                "editable"
            } else {
                "read-only"
            },
            entry.description
        ));
        rows.push(vec![
            format::date_time(entry.start, calendar),
            crate::render::output::words(entry.kind.as_str()),
            entry.description.clone(),
            if editable(entry) {
                "Editable"
            } else {
                "Read only"
            }
            .into(),
            token,
        ]);
    }
    context.table(
        "📝 Recorded entries",
        &["When", "Kind", "Details", "Editing", "Entry ID"],
        &rows,
        &machine,
    );
}

/// Puts the entries on the screen and waits for one to be picked.
///
/// With no terminal there is nobody to pick, and the failure names the flag
/// that would have answered, as every question in this tool does.
fn choose(context: &Context, entries: &[Entry], calendar: &Calendar) -> Result<Option<String>> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "no entry to change: pass --id <ENTRY> (stdin is not a terminal, so I cannot show \
             you the list; `edit --list` prints it)"
        );
    }
    if entries.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    let now = now_seconds();
    let today = format::day_short(calendar.day_of(now));
    let chosen = Listing::new(
        "Change which entry?",
        "entries",
        &crate::render::log::COLUMNS,
    )
    .rows(crate::render::log::rows(entries, calendar, now, &|entry| {
        (!editable(entry)).then(|| {
            format!(
                "this {} has no stored location, so it cannot be changed",
                entry.title.to_lowercase()
            )
        })
    }))
    .today(|heading| heading == today)
    .empty("nothing logged in this window")
    .verb("j/k or ↑/↓ move · / searches · enter edits · q leaves")
    .choose(context.output_theme())?;

    Ok(chosen)
}

/// The live selector and history references use distinct save paths.
fn entry_token(entry: &Entry) -> Option<String> {
    if entry.id == live::TOKEN {
        Some(live::TOKEN.into())
    } else {
        entry.at.as_ref().map(edit::token_for)
    }
}

/// Every located history entry supports at least a time correction.
fn editable(entry: &Entry) -> bool {
    entry.id == live::TOKEN || entry.at.is_some()
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::types::LiveState;

    #[test]
    fn every_located_history_kind_allows_time_edits() {
        for kind in crate::domain::log::Kind::ALL {
            let entry = Entry {
                id: "entry".into(),
                at: Some(RowRef::loose(kind.as_str(), "entry")),
                kind,
                start: 1000.0,
                title: "Entry".into(),
                description: String::new(),
                notes: None,
            };
            assert!(editable(&entry), "{kind:?} must allow a time correction");
        }
    }

    #[test]
    fn ongoing_sleep_is_selectable_without_a_history_row() {
        let live = LiveState {
            sleep_active: true,
            sleep_start: Some(1000.0),
            ..LiveState::default()
        };
        let entry = live::entry(&live, 1600.0).expect("ongoing sleep");
        assert!(editable(&entry));
        assert_eq!(entry_token(&entry).as_deref(), Some("sleep/current"));
        assert!(
            entry.at.is_none(),
            "a timer must not be addressed as history"
        );
        let calendar = Calendar::new("UTC").unwrap();
        let rows = crate::render::log::rows(&[entry], &calendar, 1600.0, &|_| None);
        assert_eq!(rows[0].key, "sleep/current");
        assert!(rows[0].selectable);
    }

    #[test]
    fn completed_timers_are_not_offered_and_paused_ones_remain_editable() {
        let mut state = LiveState {
            sleep_start: Some(1000.0),
            ..LiveState::default()
        };
        assert!(live::entry(&state, 1600.0).is_none());
        state.sleep_active = true;
        state.sleep_paused = true;
        let entry = live::entry(&state, 1600.0).unwrap();
        assert!(editable(&entry));
        assert!(entry.description.contains("paused"));
    }
}

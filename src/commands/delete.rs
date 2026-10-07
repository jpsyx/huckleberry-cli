//! `delete`: taking an entry off the record.
//!
//! The same list `edit` shows, and the same two paths through it: a person
//! picks an entry and confirms, a script passes `--id` and `--yes`. It removes
//! anything that came from Huckleberry, not only what this tool can log,
//! because taking a row away needs no knowledge of what is in it.
//!
//! Deleting is the one thing here that cannot be undone, so it is the one
//! thing that asks twice: the entry is described back before the question, and
//! with no terminal the failure names `--yes` rather than assuming consent.

use anyhow::{Context as _, Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Huckleberry, RowRef};

use crate::cli::{DeleteOptions, Units};
use crate::domain::log::Entry;
use crate::domain::{Calendar, log};
use crate::edit;
use crate::listing::Listing;
use crate::prompt;
use crate::render::format;
use crate::session::Context;

/// Runs the command.
pub async fn run(context: &Context, options: &DeleteOptions) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let calendar = context.calendar()?;

    // The escape hatch: a tracker whose rows the stream does not show, which
    // is how a growth measurement or a temperature is reached at all.
    if let Some(name) = &options.tracker {
        return raw(context, &client, &cid, name, options, &calendar).await;
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
    let entries: Vec<Entry> = log::build(&dataset, |amount| {
        format::volume(amount, Units::from_setting(&context.config.units))
    })
    .into_iter()
    .take(options.limit)
    .collect();

    if options.list {
        for entry in &entries {
            let Some(at) = &entry.at else { continue };
            crate::render::print(&[format!(
                "{}\t{}\t{} {}\t{}",
                edit::token_for(at),
                entry.kind.as_str(),
                format::day_short(calendar.day_of(entry.start)),
                format::clock(entry.start, &calendar),
                entry.description
            )]);
        }
        return Ok(());
    }

    let chosen = match options.id.as_deref() {
        Some(token) => {
            let at = edit::parse_token(token)?;
            let entry = entries
                .iter()
                .find(|entry| entry.at.as_ref() == Some(&at))
                .cloned();
            Some((at, entry))
        }
        None => choose(context, &entries, &calendar)?.map(|at| {
            let entry = entries
                .iter()
                .find(|entry| entry.at.as_ref() == Some(&at))
                .cloned();
            (at, entry)
        }),
    };
    let Some((at, entry)) = chosen else {
        context.report("Nothing deleted.");
        return Ok(());
    };

    let described = entry.as_ref().map_or_else(
        || format!("the entry at {}", edit::token_for(&at)),
        |entry| {
            format!(
                "the {} at {} · {}",
                entry.title.to_lowercase(),
                format::clock(entry.start, &calendar),
                entry.description
            )
        },
    );
    let started_at = match entry.as_ref() {
        Some(entry) => entry.start,
        // A token for something outside the window: find it where it lives, so
        // the tracker's summaries can be matched against the right moment.
        None => started_at(&client, &cid, &at)
            .await?
            .with_context(|| format!("no entry `{}` on the record", edit::token_for(&at)))?,
    };

    remove(
        context,
        &client,
        &cid,
        &at,
        started_at,
        &described,
        options.yes,
    )
    .await
}

/// The rows of one tracker by name, for the entries the stream does not show.
async fn raw(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    tracker: &str,
    options: &DeleteOptions,
    calendar: &Calendar,
) -> Result<()> {
    if options.list {
        context.narrate(&format!("Reading the {tracker} tracker..."));
        for (at, row) in client
            .located_rows(tracker, cid, "reading the tracker")
            .await?
            .iter()
            .rev()
        {
            let when = row
                .get("start")
                .and_then(serde_json::Value::as_f64)
                .map_or_else(
                    || "?".to_owned(),
                    |start| {
                        format!(
                            "{} {}",
                            format::day_short(calendar.day_of(start)),
                            format::clock(start, calendar)
                        )
                    },
                );
            crate::render::print(&[format!("{}\t{when}\t{row}", edit::token_for(at))]);
        }
        return Ok(());
    }

    let at = if let Some(token) = &options.id {
        edit::parse_token(token)?
    } else {
        if !prompt::available() {
            bail!("no entry to delete: pass --id <ENTRY>, or --list to see them");
        }
        let rows = client
            .located_rows(tracker, cid, "reading the tracker")
            .await?;
        super::persist_session(context, client).await?;
        let columns = [
            crate::listing::Column::new("When", crate::listing::Role::Key),
            crate::listing::Column::new("Details", crate::listing::Role::Value),
        ];
        let selected = Listing::new("Delete which tracker entry?", "entries", &columns)
            .rows(create_tracker_rows(&rows, calendar))
            .choose(context.output_theme())?;
        let Some(token) = selected else {
            return Ok(());
        };
        edit::parse_token(&token)?
    };
    let token = edit::token_for(&at);
    let started_at = started_at(client, cid, &at)
        .await?
        .with_context(|| format!("no entry `{token}` on the record"))?;
    let described = format!(
        "the {} entry at {}",
        at.tracker,
        format::clock(started_at, calendar)
    );
    remove(
        context,
        client,
        cid,
        &at,
        started_at,
        &described,
        options.yes,
    )
    .await
}

/// The moment a row claims, found where the row lives.
///
/// Also the check that it is there at all: deleting something that has already
/// gone would otherwise report success and change nothing.
async fn started_at(client: &Huckleberry, cid: &str, at: &RowRef) -> Result<Option<f64>> {
    Ok(client
        .located_rows(&at.tracker, cid, "finding the entry")
        .await?
        .into_iter()
        .find(|(found, _)| found == at)
        .and_then(|(_, row)| row.get("start").and_then(serde_json::Value::as_f64)))
}

/// Asks, then deletes.
#[allow(
    clippy::too_many_arguments,
    reason = "one command's arguments, threaded to the one place that uses them"
)]
async fn remove(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    at: &RowRef,
    started_at: f64,
    described: &str,
    yes: bool,
) -> Result<()> {
    if !yes {
        if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
            bail!(
                "deleting {described} cannot be undone: pass --yes to say so (stdin is not a \
                 terminal, so I cannot ask)"
            );
        }
        context.attention(&format!("About to delete {described}."));
        if !prompt::confirm("This cannot be undone. Delete it?", false, context.theme)? {
            context.report("Nothing deleted.");
            return Ok(());
        }
    }

    client.delete_history_row(cid, at, started_at).await?;
    super::persist_session(context, client).await?;
    context.report(&format!("Deleted {described}."));
    crate::render::print(&[format!("deleted\t{}", edit::token_for(at))]);
    Ok(())
}

/// Puts the entries on the screen and waits for one to be picked.
fn choose(context: &Context, entries: &[Entry], calendar: &Calendar) -> Result<Option<RowRef>> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "no entry to delete: pass --id <ENTRY> (stdin is not a terminal, so I cannot show \
             you the list; `delete --list` prints it)"
        );
    }
    if entries.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    let now = now_seconds();
    let today = format::day_short(calendar.day_of(now));
    // Anything that came from Huckleberry can be removed, because taking a row
    // away needs no knowledge of what is in it.
    let chosen = Listing::new(
        "Delete which entry?",
        "entries",
        &crate::render::log::COLUMNS,
    )
    .rows(crate::render::log::rows(entries, calendar, now, &|entry| {
        entry
            .at
            .is_none()
            .then(|| "that entry came from a snapshot rather than from Huckleberry".to_owned())
    }))
    .today(|heading| heading == today)
    .empty("nothing logged in this window")
    .verb("↑/↓ j/k w/s move · / searches · enter deletes · h/a or q leaves")
    .choose(context.output_theme())?;

    chosen.map(|token| edit::parse_token(&token)).transpose()
}

/// Turns raw tracker records into selectable rows without changing their identity.
#[must_use]
pub fn create_tracker_rows(
    rows: &[(RowRef, serde_json::Value)],
    calendar: &Calendar,
) -> Vec<crate::listing::Row> {
    rows.iter()
        .rev()
        .map(|(at, row)| {
            let when = row
                .get("start")
                .and_then(serde_json::Value::as_f64)
                .map_or_else(
                    || "?".to_owned(),
                    |start| {
                        format!(
                            "{} {}",
                            format::day_short(calendar.day_of(start)),
                            format::clock(start, calendar)
                        )
                    },
                );
            crate::listing::Row::new(edit::token_for(at), [when, row.to_string()])
        })
        .collect()
}

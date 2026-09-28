//! `edit`: changing an entry that is already on the record.
//!
//! The two audiences meet here the way they do everywhere else in this tool.
//! A person runs `edit`, picks an entry off a full-screen list, and is asked
//! about every field with what is already there as the answer to keep. A
//! script runs `edit --list` to find the entry it means and
//! `edit --id <ENTRY> --set key=value` to change it, and never meets a prompt.
//!
//! This module owns the terminal, the keyboard and the network. What an edit
//! *is* lives in [`crate::edit`], the list and the cursor in
//! [`crate::listing`], and the questions in [`form`].
//!
//! One thing it deliberately cannot do: move an entry in time. A row's id in
//! Huckleberry leads with its own millisecond timestamp, so changing when
//! something happened would leave history sorted by a time the row no longer
//! claims. Delete it in the app and log it again.

pub mod form;

use anyhow::{Context as _, Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{DiaperDetails, Huckleberry, RowRef};

use crate::cli::{Amount, Colour, Consistency, EditOptions, PottyOutcome, Units};
use crate::domain::log::Entry;
use crate::domain::{Calendar, log};
use crate::edit::{self, Draft};
use crate::listing::Listing;
use crate::render::format;
use crate::session::Context;

/// Runs the command.
pub async fn run(context: &Context, options: &EditOptions) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
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
    let entries: Vec<Entry> = log::build(&dataset)
        .into_iter()
        .take(options.limit)
        .collect();

    if options.list {
        list(context, &entries, &calendar);
        return Ok(());
    }

    let picked = match options.id.as_deref() {
        Some(token) => Some(edit::parse_token(token)?),
        None => choose(context, &entries, &calendar)?,
    };
    // Leaving the list without picking is not a failure: nothing was asked
    // for, so nothing happened and nothing is reported as having gone wrong.
    let Some(at) = picked else {
        context.report("Nothing changed.");
        return Ok(());
    };

    let units = Units::from_setting(&context.config.units);
    let mut draft = edit::draft_for(&dataset, &at, units).with_context(|| {
        format!(
            "no entry `{}` in the last {window} days: `edit --list` says what there is",
            edit::token_for(&at)
        )
    })?;

    // Kept so the confirmation can say what changed rather than only what the
    // entry now says, which reads as the old value when nothing moved.
    let before = draft.clone();
    if options.set.is_empty() {
        form::fill(context, &client, &cid, &mut draft).await?;
    } else {
        draft.apply(&options.set)?;
    }

    if draft == before {
        context.report(&format!(
            "The {} is unchanged: {}.",
            draft.what(),
            edit::summary(&draft)
        ));
        return Ok(());
    }

    save(&client, &cid, &at, &draft).await?;
    super::persist_session(context, &client).await?;
    context.receipt(
        "📝 Entry updated",
        &[
            ("Kind", crate::render::output::words(draft.what())),
            ("Before", edit::summary(&before)),
            ("After", edit::summary(&draft)),
            ("Entry ID", edit::token_for(&at)),
        ],
        &[format!("entry\t{}", edit::token_for(&at))],
    );
    Ok(())
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
        let Some(at) = &entry.at else {
            continue;
        };
        machine.push(format!(
            "{}\t{}\t{} {}\t{}\t{}",
            edit::token_for(at),
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
            edit::token_for(at),
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
fn choose(context: &Context, entries: &[Entry], calendar: &Calendar) -> Result<Option<RowRef>> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "no entry to change: pass --id <ENTRY> (stdin is not a terminal, so I cannot show \
             you the list; `edit --list` prints it)"
        );
    }
    if entries.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    let today = format::day_short(calendar.day_of(now_seconds()));
    let chosen = Listing::new(
        "Change which entry?",
        "entries",
        &crate::render::log::COLUMNS,
    )
    .rows(crate::render::log::rows(entries, calendar, &|entry| {
        (!editable(entry)).then(|| {
            format!(
                "this tool does not log a {}, so it cannot change one",
                entry.title.to_lowercase()
            )
        })
    }))
    .today(|heading| heading == today)
    .empty("nothing logged in this window")
    .verb("j/k or ↑/↓ move · / searches · enter edits · q leaves")
    .choose(context.output_theme())?;

    chosen.map(|token| edit::parse_token(&token)).transpose()
}

/// Whether this tool can change an entry.
///
/// It edits what it can log: a diaper, a potty trip, a bottle, a nursing
/// session, a meal and a sleep. A pumping session and a milestone are listed
/// anyway, because the stream is the stream, and saying so is better than
/// leaving somebody to wonder where their entry went.
const fn editable(entry: &Entry) -> bool {
    entry.at.is_some()
        && matches!(
            entry.kind,
            crate::domain::log::Kind::Sleep
                | crate::domain::log::Kind::Feed
                | crate::domain::log::Kind::Diaper
        )
}
/// Writes the draft back to the row it came from.
async fn save(client: &Huckleberry, cid: &str, at: &RowRef, draft: &Draft) -> Result<()> {
    match draft {
        Draft::Diaper(diaper) => {
            let details = DiaperDetails {
                pee_amount: diaper.pee.map(Amount::to_api),
                poo_amount: diaper.poo.map(Amount::to_api),
                color: diaper.color.map(Colour::to_api),
                consistency: diaper.consistency.map(Consistency::to_api),
                rash: diaper.rash,
                notes: diaper.notes.clone(),
            };
            client
                .update_diaper_entry(
                    cid,
                    at,
                    diaper.mode.to_api(),
                    &details,
                    diaper.how.map(PottyOutcome::to_api),
                )
                .await?;
        }
        Draft::Bottle(bottle) => {
            client
                .update_bottle_entry(
                    cid,
                    at,
                    bottle.amount,
                    bottle.kind.to_api(),
                    bottle.units.to_api(),
                    bottle.notes.as_deref(),
                )
                .await?;
        }
        Draft::Nursing(nursing) => {
            client
                .update_nursing_entry(
                    cid,
                    at,
                    nursing.left_minutes * 60.0,
                    nursing.right_minutes * 60.0,
                    nursing.notes.as_deref(),
                )
                .await?;
        }
        Draft::Solids(meal) => {
            // The family's own foods, so that correcting a meal does not turn
            // a food that is on their list into one that is not.
            let known = client.custom_foods(cid, false).await.unwrap_or_default();
            let references: Vec<_> = meal
                .foods
                .iter()
                .map(|food| super::feed::match_food(food, &known, &meal.amount))
                .collect();
            client
                .update_solids_entry(
                    cid,
                    at,
                    &references,
                    meal.reaction.map(crate::cli::Reaction::to_api),
                    meal.notes.as_deref(),
                )
                .await?;
        }
        Draft::Sleep(sleep) => {
            client
                .update_sleep_entry(cid, at, sleep.minutes * 60.0, sleep.notes.as_deref())
                .await?;
        }
    }
    Ok(())
}

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
//! [`crate::picker`], and the questions in [`form`].
//!
//! One thing it deliberately cannot do: move an entry in time. A row's id in
//! Huckleberry leads with its own millisecond timestamp, so changing when
//! something happened would leave history sorted by a time the row no longer
//! claims. Delete it in the app and log it again.

pub mod form;

use anyhow::{Context as _, Result, bail};
use crossterm::event::{Event, KeyEventKind};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{DiaperDetails, Huckleberry, RowRef};

use crate::cli::{Amount, Colour, Consistency, PottyOutcome, Units};
use crate::domain::log::Entry;
use crate::domain::{Calendar, log};
use crate::edit::{self, Draft};
use crate::picker::{Action, Picker, action_for, draw};
use crate::render::format;
use crate::session::Context;

/// Runs the command.
pub async fn run(
    context: &Context,
    id: Option<&str>,
    set: &[String],
    listing: bool,
    days: Option<u32>,
    limit: usize,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let window = context.days(days);
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
    let entries: Vec<Entry> = log::build(&dataset).into_iter().take(limit).collect();

    if listing {
        list(&entries, &calendar);
        return Ok(());
    }

    let picked = match id {
        Some(token) => Some(edit::parse_token(token)?),
        None => choose(entries, &calendar)?,
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

    if set.is_empty() {
        form::fill(context, &client, &cid, &mut draft).await?;
    } else {
        draft.apply(set)?;
    }

    save(&client, &cid, &at, &draft).await?;
    super::persist_session(context, &client).await?;
    context.report(&format!(
        "Changed the {}: {}.",
        draft.what(),
        edit::summary(&draft)
    ));
    println!("entry\t{}", edit::token_for(&at));
    Ok(())
}

/// Prints the entries, one `key<TAB>value` row each, for a script to read.
///
/// On stdout, because this is the answer the command was asked for. An entry
/// this tool cannot change says so in its own column rather than being left
/// out, so a script does not have to guess why its id is missing.
fn list(entries: &[Entry], calendar: &Calendar) {
    for entry in entries {
        let Some(at) = &entry.at else {
            continue;
        };
        println!(
            "{}\t{}\t{} {}\t{}\t{}",
            edit::token_for(at),
            entry.kind.as_str(),
            format::day_short(calendar.day_of(entry.start)),
            format::clock(entry.start, calendar),
            if crate::picker::editable(entry) {
                "editable"
            } else {
                "read-only"
            },
            entry.description
        );
    }
}

/// Puts the entries on the screen and waits for one to be picked.
///
/// With no terminal there is nobody to pick, and the failure names the flag
/// that would have answered, as every question in this tool does.
fn choose(entries: Vec<Entry>, calendar: &Calendar) -> Result<Option<RowRef>> {
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!(
            "no entry to change: pass --id <ENTRY> (stdin is not a terminal, so I cannot show \
             you the list; `edit --list` prints it)"
        );
    }
    if entries.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    let mut picker = Picker::new(entries);

    // As the dashboard: `ratatui::init` installs a panic hook that restores
    // the terminal, so a panic leaves a usable shell behind.
    let mut terminal = ratatui::init();
    let chosen = pick(&mut terminal, &mut picker, calendar);
    ratatui::restore();
    let chosen = chosen?;

    chosen
        .map(|entry| {
            entry
                .at
                .context("that entry did not come from Huckleberry, so there is nothing to change")
        })
        .transpose()
}

/// The keyboard loop, with the terminal already set up.
fn pick(
    terminal: &mut ratatui::DefaultTerminal,
    picker: &mut Picker,
    calendar: &Calendar,
) -> Result<Option<Entry>> {
    loop {
        terminal
            .draw(|frame| draw(frame, picker, calendar))
            .context("drawing the list")?;

        let Event::Key(key) = crossterm::event::read().context("reading a keystroke")? else {
            continue;
        };
        // Windows reports press and release; acting on both would move two
        // entries for one keystroke.
        if key.kind != KeyEventKind::Press {
            continue;
        }
        // Any keystroke clears the last complaint: it was about the one
        // before it.
        picker.trouble = None;
        match action_for(key) {
            Action::Cancel => return Ok(None),
            Action::Move(delta) => picker.move_by(delta),
            Action::First => picker.first(),
            Action::Last => picker.last(),
            Action::Take => {
                if let Some(entry) = picker.take() {
                    return Ok(Some(entry));
                }
            }
            Action::Ignore => {}
        }
    }
}

/// Writes the draft back to the row it came from.
async fn save(client: &Huckleberry, cid: &str, at: &RowRef, draft: &Draft) -> Result<()> {
    match draft {
        Draft::Nappy(nappy) => {
            let details = DiaperDetails {
                pee_amount: nappy.pee.map(Amount::to_api),
                poo_amount: nappy.poo.map(Amount::to_api),
                color: nappy.color.map(Colour::to_api),
                consistency: nappy.consistency.map(Consistency::to_api),
                rash: nappy.rash,
                notes: nappy.notes.clone(),
            };
            client
                .update_diaper_entry(
                    cid,
                    at,
                    nappy.mode.to_api(),
                    &details,
                    nappy.how.map(PottyOutcome::to_api),
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

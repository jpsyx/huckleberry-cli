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

use anyhow::{Result, bail};
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Huckleberry, RowRef};

use crate::cli::{DeleteOptions, Units};
use crate::domain::{Calendar, log};
use crate::edit;
use crate::prompt;
use crate::render::format;
use crate::session::Context;

mod prepare;
use super::delete_form::{DeleteForm, DeleteSubmission};
use super::prepared::{CommandProgress, PreparedCommand};

/// Uses the same retained confirmation form in the shell and on a terminal.
pub async fn run(context: &Context, options: &DeleteOptions) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    if options.list {
        return list(context, &client, &cid, options).await;
    }
    if options.id.is_none() && !prompt::available() {
        bail!("no entry to delete: pass --id <ENTRY>, or --list to see them");
    }
    if !options.yes && !prompt::available() {
        bail!("deleting cannot be undone: pass --yes to say so");
    }
    let answer = prompt::flow::runner::read(
        prepare::create_model(context, &client, &cid, options).await?,
        now_seconds(),
        context.theme,
    )?;
    execute(context, &client, &cid, answer).await
}
/// Prepares a deletion with a selected account, before any destructive operation.
pub(crate) async fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    options: DeleteOptions,
    now: f64,
) -> Result<PreparedCommand> {
    let model = prepare::create_model(&context, &client, &cid, &options).await?;
    prepare_model(context, client, cid, model, now)
}
/// Binds an owned model to one consuming deletion operation.
pub(crate) fn prepare_model(
    context: Context,
    client: Huckleberry,
    cid: String,
    model: DeleteForm,
    now: f64,
) -> Result<PreparedCommand> {
    PreparedCommand::create_prepared(model, now, move |answer| async move {
        execute(&context, &client, &cid, answer).await?;
        Ok(CommandProgress::Finished { home: false })
    })
}
async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: DeleteSubmission,
) -> Result<()> {
    client
        .delete_history_row(cid, &answer.at, answer.started)
        .await?;
    super::persist_session(context, client).await?;
    context.report(&format!("Deleted {}.", answer.description));
    crate::render::print(&[format!("deleted\t{}", edit::token_for(&answer.at))]);
    Ok(())
}
async fn started_at(client: &Huckleberry, cid: &str, at: &RowRef) -> Result<Option<f64>> {
    Ok(client
        .located_rows(&at.tracker, cid, "finding the entry")
        .await?
        .into_iter()
        .find(|(found, _)| found == at)
        .and_then(|(_, row)| row.get("start").and_then(serde_json::Value::as_f64)))
}
async fn list(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &DeleteOptions,
) -> Result<()> {
    let calendar = context.calendar()?;
    if let Some(tracker) = &options.tracker {
        for row in create_tracker_rows(
            &client
                .located_rows(tracker, cid, "reading the tracker")
                .await?,
            &calendar,
        ) {
            crate::render::print(&[format!("{}\t{}", row.key, row.cells.join("\t"))]);
        }
    } else {
        let dataset = crate::dataset::pull(
            client,
            cid,
            None,
            context.days(options.days),
            &context.config.timezone,
            now_seconds(),
        )
        .await?;
        for entry in log::build(&dataset, |amount| {
            format::volume(amount, Units::from_setting(&context.config.units))
        })
        .into_iter()
        .take(options.limit)
        {
            if let Some(at) = entry.at {
                crate::render::print(&[format!(
                    "{}\t{}\t{} {}\t{}",
                    edit::token_for(&at),
                    entry.kind.as_str(),
                    format::day_short(calendar.day_of(entry.start)),
                    format::clock(entry.start, &calendar),
                    entry.description
                )]);
            }
        }
    }
    super::persist_session(context, client).await
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

//! Read-only preparation of deletion targets and confirmation descriptions.
use crate::{
    cli::{DeleteOptions, Units},
    commands::delete_form::{DeleteForm, DeleteTarget},
    domain::{Calendar, log},
    edit,
    listing::{Column, Role},
    render::format,
    session::Context,
};
use anyhow::{Context as _, Result, bail};
use huckleberry_api::{Huckleberry, client::now_seconds};
const RAW_COLUMNS: &[Column] = &[
    Column::new("When", Role::Key),
    Column::new("Details", Role::Value),
];
pub(super) async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &DeleteOptions,
) -> Result<DeleteForm> {
    let calendar = context.calendar()?;
    let (rows, mut targets, columns) = if let Some(tracker) = &options.tracker {
        let raw = client
            .located_rows(tracker, cid, "reading the tracker")
            .await?;
        let rows = super::create_tracker_rows(&raw, &calendar);
        let targets = raw
            .into_iter()
            .filter_map(|(at, row)| {
                Some(DeleteTarget {
                    started: row.get("start")?.as_f64()?,
                    description: format!("the {} entry {}", at.tracker, edit::token_for(&at)),
                    at,
                })
            })
            .collect();
        (rows, targets, RAW_COLUMNS)
    } else {
        history(context, client, cid, options, &calendar).await?
    };
    if let Some(token) = &options.id
        && !targets
            .iter()
            .any(|target| edit::token_for(&target.at) == *token)
    {
        let at = edit::parse_token(token)?;
        let started = super::started_at(client, cid, &at)
            .await?
            .with_context(|| format!("no entry `{token}` on the record"))?;
        targets.push(DeleteTarget {
            at,
            started,
            description: format!("the entry at {token}"),
        });
    }
    if targets.is_empty() {
        bail!("nothing logged in this window: try a longer --days");
    }
    Ok(DeleteForm::create_delete(
        rows,
        columns,
        targets,
        options.id.clone(),
        options.yes,
    ))
}
async fn history(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    options: &DeleteOptions,
    calendar: &Calendar,
) -> Result<(
    Vec<crate::listing::Row>,
    Vec<DeleteTarget>,
    &'static [Column],
)> {
    let now = now_seconds();
    let dataset = crate::dataset::pull(
        client,
        cid,
        None,
        context.days(options.days),
        &context.config.timezone,
        now,
    )
    .await?;
    let entries = log::build(&dataset, |amount| {
        format::volume(amount, Units::from_setting(&context.config.units))
    })
    .into_iter()
    .take(options.limit)
    .collect::<Vec<_>>();
    let rows = crate::render::log::rows(&entries, calendar, now, &|entry| {
        entry
            .at
            .is_none()
            .then(|| "that entry came from a snapshot rather than from Huckleberry".into())
    });
    let targets = entries
        .into_iter()
        .filter_map(|entry| {
            Some(DeleteTarget {
                at: entry.at?,
                started: entry.start,
                description: format!(
                    "the {} at {} · {}",
                    entry.title.to_lowercase(),
                    format::clock(entry.start, calendar),
                    entry.description
                ),
            })
        })
        .collect();
    Ok((rows, targets, &crate::render::log::COLUMNS))
}

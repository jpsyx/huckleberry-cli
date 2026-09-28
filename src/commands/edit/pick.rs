//! Read-only entry selection for pending interactive edit options.
use crate::{
    cli::Units,
    domain::{Calendar, log},
    edit::{self, Draft},
    session::Context,
};
use anyhow::{Result, anyhow};
use huckleberry_api::client::now_seconds;

/// An entry and its editable shape; no write has occurred.
pub struct SelectedEdit {
    /// Canonical selector.
    pub token: String,
    /// Details when this CLI has a form for them.
    pub draft: Option<Draft>,
    /// Current event time.
    pub started: f64,
}

/// Selects or resolves an entry using the same history and live rows as edit.
pub async fn select(
    context: &Context,
    id: Option<&str>,
    days: Option<u32>,
    limit: usize,
) -> Result<SelectedEdit> {
    let (client, cid) = crate::commands::client_and_child(context).await?;
    let window = context.days(days);
    let dataset = crate::dataset::pull(
        &client,
        &cid,
        None,
        window,
        &context.config.timezone,
        now_seconds(),
    )
    .await?;
    crate::commands::persist_session(context, &client).await?;
    let calendar = Calendar::new(&dataset.timezone)?;
    let mut entries = log::build(&dataset);
    super::history::add_health(context, &client, &cid, window, &mut entries).await?;
    entries.truncate(limit);
    if let Some(live) = super::live::entry(&dataset.live, now_seconds()) {
        entries.insert(0, live);
    }
    let token = match id {
        Some(id) => id.to_owned(),
        None => super::choose(context, &entries, &calendar)?.ok_or(crate::prompt::Cancelled)?,
    };
    let entry = entries
        .iter()
        .find(|entry| super::entry_token(entry).as_deref() == Some(&token))
        .ok_or_else(|| anyhow!("entry is outside this window; increase days or limit"))?;
    let draft = entry
        .at
        .as_ref()
        .and_then(|at| edit::draft_for(&dataset, at, Units::from_setting(&context.config.units)));
    Ok(SelectedEdit {
        token,
        draft,
        started: entry.start,
    })
}

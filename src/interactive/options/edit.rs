//! Builds repeated field changes without typing field names or KEY=VALUE.
use crate::{interactive::draft::CommandDraft, session::Context};
use anyhow::Result;

pub(super) async fn collect(context: &Context, pending: &mut CommandDraft) -> Result<()> {
    let selected = crate::commands::edit::pick::select(
        context,
        pending
            .values
            .get("id")
            .and_then(|values| values.first())
            .map(String::as_str),
        pending
            .values
            .get("days")
            .and_then(|values| values.first())
            .and_then(|value| value.parse().ok()),
        pending
            .values
            .get("limit")
            .and_then(|values| values.first())
            .and_then(|value| value.parse().ok())
            .unwrap_or(40),
    )
    .await?;
    let changes = crate::commands::edit::form::collect(
        context,
        selected.draft.as_ref(),
        selected.started,
        selected.token == "sleep/current",
        pending.values.get("set").cloned().unwrap_or_default(),
    )
    .await?;
    pending.set("id", vec![selected.token]);
    pending.set("set", changes);
    Ok(())
}

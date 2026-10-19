//! One explicit action shared by the field menu and `--set resume=true`.
use crate::{edit, session::Context};
use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;

/// Reject mixed changes before any detail or timer write can occur.
pub(super) fn requested(changes: &[String]) -> Result<bool> {
    let requested = changes.iter().any(|change| {
        change
            .split_once('=')
            .is_some_and(|(key, _)| key.trim().eq_ignore_ascii_case("resume"))
    });
    if requested && (changes.len() != 1 || changes[0] != "resume=true") {
        bail!("resume a completed sleep with --set resume=true alone; save other edits separately");
    }
    Ok(requested)
}

/// Read failures hide the optional action without making ordinary editing unavailable.
pub(super) async fn available(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
) -> Option<String> {
    match client.resumable_sleep(cid).await {
        Ok(at) => at.as_ref().map(edit::token_for),
        Err(error) => {
            context.warn(&format!(
                "Cannot check whether sleep can be resumed: {}",
                crate::dataset::describe(&error)
            ));
            None
        }
    }
}

/// The API rechecks all histories before replacing the completed entry.
pub(super) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: super::retained::EditSubmission,
) -> Result<()> {
    if answer.started.is_some() || answer.target.timer.is_some() {
        bail!("resume a completed sleep separately from other edits");
    }
    client
        .resume_completed_sleep(cid, &edit::parse_token(&answer.target.token)?)
        .await?;
    super::super::persist_session(context, client).await?;
    context.report("😴 Sleep resumed from its recorded start.");
    Ok(())
}

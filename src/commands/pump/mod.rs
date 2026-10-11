//! Pump logging and timer actions.

pub mod form;
mod logging;
pub(crate) mod prepare;
mod prompts;
mod timer;

#[cfg(test)]
mod tests;

use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;

use crate::cli::PumpAction;
use crate::session::Context;

/// Runs the chosen pumping action.
pub async fn run(context: &Context, action: &PumpAction) -> Result<()> {
    if context.offline.is_some() {
        bail!("`--offline` reads a snapshot, so pump actions need a live account");
    }
    let client = context.client()?;
    let outcome = async {
        let cid = super::which_child(context, &client).await?;
        act(context, &client, &cid, action).await
    }
    .await;
    let persisted = super::persist_session(context, &client).await;
    outcome?;
    persisted
}

/// Keeps fallible actions inside the session-persistence boundary.
async fn act(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    action: &PumpAction,
) -> Result<()> {
    if matches!(action, PumpAction::Status) {
        return timer::status(context, client, cid).await;
    }
    let answer = prepare::collect(context, client, cid, action).await?;
    execute(context, client, cid, answer).await
}

async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: form::PumpSubmission,
) -> Result<()> {
    if matches!(
        answer.operation,
        form::PumpOperation::Log | form::PumpOperation::Stop
    ) {
        logging::execute(context, client, cid, answer).await
    } else {
        timer::execute(context, client, cid, answer).await
    }
}

//! Pump logging and timer actions.

mod logging;
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
    match action {
        PumpAction::Log { options } => logging::log(context, client, cid, options).await,
        PumpAction::Stop { at, values } => {
            logging::stop(context, client, cid, at.as_deref(), values).await
        }
        _ => timer::run(context, client, cid, action).await,
    }
}

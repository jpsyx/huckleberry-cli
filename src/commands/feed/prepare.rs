//! Owned clients and forms let the shell suspend without keeping a worker alive.
use super::{BottleInput, bottle, nursing};
use crate::{
    cli::FeedAction,
    commands::prepared::{CommandProgress, PreparedCommand},
    session::Context,
};
use anyhow::{Result, bail};
use huckleberry_api::Huckleberry;

/// Prepares a feeding form with its already selected client and child.
pub async fn prepare_with_client(
    context: Context,
    client: Huckleberry,
    cid: String,
    action: FeedAction,
    now: f64,
) -> Result<PreparedCommand> {
    match action {
        FeedAction::Bottle {
            at,
            amount,
            bottle_type,
            units,
            notes,
        } => {
            let model = bottle::create_model(
                &context,
                &client,
                &cid,
                BottleInput {
                    at: at.as_deref(),
                    amount,
                    bottle_type,
                    units,
                    notes: notes.as_deref(),
                },
            )
            .await?;
            PreparedCommand::create_prepared(model, now, move |answer| async move {
                bottle::execute(&context, &client, &cid, answer).await?;
                crate::commands::persist_session(&context, &client).await?;
                Ok(CommandProgress::Finished { home: true })
            })
        }
        FeedAction::Nursing { action } => {
            let model = nursing::create_model(&context, &client, &cid, &action).await?;
            PreparedCommand::create_prepared(model, now, move |answer| async move {
                nursing::execute(&context, &client, &cid, answer).await?;
                crate::commands::persist_session(&context, &client).await?;
                Ok(CommandProgress::Finished { home: true })
            })
        }
        FeedAction::Solids { .. } => bail!("meal form is not registered"),
    }
}

//! Background work prepares or consumes forms; it never owns a suspended draft.
use super::{Landing, settle};
use crate::{
    commands::prepared::{CommandProgress, Execution, PreparedCommand},
    interactive::{catalog::CommandPath, session::SessionOptions},
    theme::Theme,
};
use anyhow::Result;
use tokio::task::JoinHandle;

pub(super) enum Outcome {
    Continue(Box<PreparedCommand>),
    Finished(Landing),
}

pub(super) fn prepare(
    path: CommandPath,
    globals: SessionOptions,
    theme: Theme,
) -> JoinHandle<Outcome> {
    tokio::spawn(async move {
        match crate::interactive::operation::prepare(
            &path,
            &globals,
            theme,
            huckleberry_api::client::now_seconds(),
        )
        .await
        {
            Ok(Some(prepared)) => Outcome::Continue(Box::new(prepared)),
            Ok(None) => Outcome::Finished(settle(
                Box::pin(crate::interactive::operation::run(&path, &globals, theme)).await,
                theme,
            )),
            Err(error) => Outcome::Finished(settle(Err(error), theme)),
        }
    })
}

pub(super) fn execute(execution: Execution, theme: Theme) -> JoinHandle<Outcome> {
    tokio::spawn(async move { executed(execution.await, theme) })
}

fn executed(result: Result<CommandProgress>, theme: Theme) -> Outcome {
    match result {
        Ok(CommandProgress::Continue(prepared)) => Outcome::Continue(prepared),
        Ok(CommandProgress::Finished { home }) => {
            // Collection is over: dismissal cannot replay the action.
            let _ = crate::interactive::pause(theme);
            Outcome::Finished(if home { Landing::Home } else { Landing::Stay })
        }
        Err(error) => Outcome::Finished(settle(Err(error), theme)),
    }
}

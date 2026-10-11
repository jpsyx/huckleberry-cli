//! Utility preparation shares the same collectors as explicit invocations.
use super::{CommandProgress, PreparedCommand};
use crate::{
    cli::{AuthAction, ChildAction, Command, ConfigAction, FoodsAction},
    commands::utility_form::{ChildForm, FoodForm, ReadyForm, SettingsForm},
    session::Context,
};
use anyhow::Result;

pub(super) async fn prepare(
    context: Context,
    command: Command,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    match command {
        Command::Auth {
            action:
                AuthAction::Login {
                    email,
                    password,
                    timezone,
                },
        } => {
            let model = crate::commands::auth::create_model(&context, email, password, timezone)?;
            PreparedCommand::create_prepared(model, now, move |answer| async move {
                crate::commands::auth::execute(&context, answer).await?;
                Ok(CommandProgress::Finished { home: true })
            })
            .map(Some)
        }
        Command::Config {
            action: ConfigAction::Set { key, value },
        } => {
            let model = SettingsForm::create_settings(context.config.clone(), key, value)?;
            PreparedCommand::create_prepared(model, now, move |answer| async move {
                crate::commands::settings::execute(&context, answer)?;
                Ok(CommandProgress::Finished { home: false })
            })
            .map(Some)
        }
        Command::Child {
            action: ChildAction::Use { cid },
        } => {
            let client = context.client()?;
            let children = client.user().await?.child_list;
            let model =
                ChildForm::create_child(crate::commands::child::labels(&children), cid, false)?;
            PreparedCommand::create_prepared(model, now, move |answer| async move {
                crate::commands::child::execute(&context, answer)?;
                crate::commands::persist_session(&context, &client).await?;
                Ok(CommandProgress::Finished { home: false })
            })
            .map(Some)
        }
        Command::Foods {
            action: FoodsAction::Add { name },
        } => {
            let (client, cid) = crate::commands::client_and_child(&context).await?;
            PreparedCommand::create_prepared(
                FoodForm::create_food(name),
                now,
                move |answer| async move {
                    crate::commands::foods::execute(&context, &client, &cid, answer).await?;
                    Ok(CommandProgress::Finished { home: false })
                },
            )
            .map(Some)
        }
        command @ (Command::Auth {
            action: AuthAction::Logout,
        }
        | Command::Export { .. }) => immediate(context, command, now).map(Some),
        _ => Ok(None),
    }
}

pub(super) fn immediate(context: Context, command: Command, now: f64) -> Result<PreparedCommand> {
    PreparedCommand::create_prepared(ReadyForm, now, move |()| async move {
        if crate::interactive::operation::is_read_view(&command) {
            crate::interactive::operation::run_ready_view(&context, &command).await?;
            return Err(crate::prompt::Cancelled.into());
        }
        crate::commands::act(&context, &command).await?;
        Ok(CommandProgress::Finished {
            home: matches!(command, Command::Auth { .. }),
        })
    })
}

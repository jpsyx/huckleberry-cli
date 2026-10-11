//! Completed prerequisites continue once with reloaded context and the original intent.
use super::{CommandProgress, PreparedCommand, utilities};
use crate::{
    cli::{ChildAction, Command},
    commands::utility_form::{ChildForm, LoginForm, ReadyForm},
    session::Context,
    setup::{self, Needs},
};
use anyhow::Result;

/// Prepares prerequisites before collection, preserving the standalone dispatch policy.
pub async fn prepare(
    context: Context,
    command: Command,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    let recovery_context = context.reload()?;
    let requested = command.clone();
    let result = prepare_once(context, command, now).await;
    finish_preparation(recovery_context, requested, now, result, authenticate)
}

async fn prepare_once(
    context: Context,
    command: Command,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    let needs = setup::needs(&command, context.offline.is_some());
    if let Some(gate) = prerequisite(&context, Some(command.clone()), needs, now).await? {
        return Ok(Some(gate));
    }
    let mut context = context;
    if needs_child(&context, &command) {
        let children = context.client()?.user().await?.child_list;
        if let [only] = children.as_slice() {
            context.child_override = Some(only.cid.clone());
        } else {
            return child(
                context,
                command,
                crate::commands::child::labels(&children),
                now,
            )
            .map(Some);
        }
    }
    super::execute::prepare(context, command, now).await
}

/// Classifies preparation failures before any submission has been collected.
pub(super) fn finish_preparation<Authenticate, Work>(
    context: Context,
    command: Command,
    now: f64,
    result: Result<Option<PreparedCommand>>,
    authenticate: Authenticate,
) -> Result<Option<PreparedCommand>>
where
    Authenticate:
        FnOnce(Context, crate::commands::utility_form::LoginSubmission) -> Work + Send + 'static,
    Work: std::future::Future<Output = Result<Context>> + Send + 'static,
{
    if result.as_ref().is_err_and(|error| {
        error
            .downcast_ref::<huckleberry_api::Error>()
            .is_some_and(huckleberry_api::Error::is_authentication)
    }) {
        let model = crate::commands::auth::create_model(&context, None, None, None)?
            .asking_for_credentials();
        return bind_login(context, Some(command), model, now, authenticate).map(Some);
    }
    result
}

async fn authenticate(
    context: Context,
    answer: crate::commands::utility_form::LoginSubmission,
) -> Result<Context> {
    crate::commands::auth::execute(&context, answer).await?;
    context.reload()
}

/// Bare `h` can open setup inside its panel without preventing access to Home.
pub async fn startup(context: Context, now: f64) -> Result<Option<PreparedCommand>> {
    let needs = Needs {
        credentials: context.offline.is_none(),
        settings: true,
    };
    prerequisite(&context, None, needs, now).await
}

async fn prerequisite(
    context: &Context,
    command: Option<Command>,
    needs: Needs,
    now: f64,
) -> Result<Option<PreparedCommand>> {
    if needs.credentials && !context.credentials()?.is_usable() {
        return create_login(context.reload()?, command, now, authenticate).map(Some);
    }
    if needs.settings && !setup::missing(&context.config).is_empty() {
        let model = setup::form::SetupForm::create_setup(
            context.config.clone(),
            setup::run::age_in_days(context).await,
        );
        let context = context.reload()?;
        return PreparedCommand::create_prepared(model, now, move |config| async move {
            setup::run::save(&context, &config)?;
            continue_command(context.reload()?, command).await
        })
        .map(Some);
    }
    Ok(None)
}

fn continue_command(context: Context, command: Option<Command>) -> super::Execution {
    Box::pin(async move {
        let now = huckleberry_api::client::now_seconds();
        let prepared = if let Some(command) = command {
            let fallback = command.clone();
            let fallback_context = context.reload()?;
            match Box::pin(prepare(context, command, now)).await? {
                Some(prepared) => prepared,
                None => utilities::immediate(fallback_context, fallback, now)?,
            }
        } else {
            match Box::pin(startup(context, now)).await? {
                Some(prepared) => prepared,
                None => PreparedCommand::create_prepared(ReadyForm, now, |()| async {
                    Ok(CommandProgress::Finished { home: true })
                })?,
            }
        };
        Ok(CommandProgress::Continue(Box::new(prepared)))
    })
}

fn needs_child(context: &Context, command: &Command) -> bool {
    context.offline.is_none()
        && context.child_override.is_none()
        && context.config.child().is_none()
        && !matches!(
            command,
            Command::Auth { .. }
                | Command::Config { .. }
                | Command::Info { .. }
                | Command::Child {
                    action: ChildAction::List | ChildAction::Use { .. }
                }
        )
}

fn child(
    context: Context,
    command: Command,
    children: Vec<(String, String)>,
    now: f64,
) -> Result<PreparedCommand> {
    let model = ChildForm::create_child(children, None, true)?;
    PreparedCommand::create_prepared(model, now, move |answer| async move {
        let cid = answer.cid.clone();
        crate::commands::child::execute(&context, answer)?;
        let mut context = context.reload()?;
        context.child_override = Some(cid);
        continue_command(context, Some(command)).await
    })
}

/// Keeps the authentication boundary consuming and independently testable.
pub(super) fn create_login<Authenticate, Work>(
    context: Context,
    command: Option<Command>,
    now: f64,
    authenticate: Authenticate,
) -> Result<PreparedCommand>
where
    Authenticate:
        FnOnce(Context, crate::commands::utility_form::LoginSubmission) -> Work + Send + 'static,
    Work: std::future::Future<Output = Result<Context>> + Send + 'static,
{
    let model = crate::commands::auth::create_model(&context, None, None, None)?;
    bind_login(context, command, model, now, authenticate)
}

fn bind_login<Authenticate, Work>(
    context: Context,
    command: Option<Command>,
    model: LoginForm,
    now: f64,
    authenticate: Authenticate,
) -> Result<PreparedCommand>
where
    Authenticate:
        FnOnce(Context, crate::commands::utility_form::LoginSubmission) -> Work + Send + 'static,
    Work: std::future::Future<Output = Result<Context>> + Send + 'static,
{
    PreparedCommand::create_prepared(model, now, move |answer| async move {
        let context = authenticate(context, answer).await?;
        continue_command(context, command).await
    })
}

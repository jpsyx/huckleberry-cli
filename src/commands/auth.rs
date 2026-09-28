//! `auth`: signing in, checking the session, signing out.

use anyhow::Result;
use huckleberry_api::client::now_seconds;
use huckleberry_api::{Credentials, Huckleberry, auth};

use crate::cli::AuthAction;
use crate::credentials::PASSWORD_ENV;
use crate::domain::time::format_duration;
use crate::prompt::{self, Question};
use crate::session::Context;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &AuthAction) -> Result<()> {
    match action {
        AuthAction::Login {
            email,
            password,
            timezone,
        } => login(context, email.clone(), password.clone(), timezone.clone()).await,
        AuthAction::Status => status(context),
        AuthAction::Logout => logout(context),
    }
}

/// Signs in and saves the session.
async fn login(
    context: &Context,
    email: Option<String>,
    password: Option<String>,
    timezone: Option<String>,
) -> Result<()> {
    let from_environment = std::env::var(PASSWORD_ENV)
        .ok()
        .is_some_and(|value| !value.is_empty());
    let (email, password) = context.ask_for_credentials(email, password)?;
    let timezone = resolve_timezone(context, timezone)?;

    context.narrate(&format!("Signing in as {email}..."));
    let client = Huckleberry::new(Credentials::new(&email, &password), &timezone)?;
    let session = client.authenticate().await?;

    let mut resolved = context.credentials()?;
    resolved.email = Some(email.clone());
    resolved.password = Some(password);
    resolved.password_from_environment = from_environment;
    resolved.session = Some(session);
    context.save_credentials(&resolved)?;

    if context.config.timezone != timezone {
        let mut config = context.config.clone();
        config.set("timezone", &timezone)?;
        crate::config::save(&context.config_path, &config)?;
        context.detail(&format!("saved timezone = {timezone}"));
    }

    context.report(&format!("Signed in as {email}."));
    if from_environment {
        context.detail(&format!(
            "the password came from {PASSWORD_ENV}, so it was not written to disk"
        ));
    } else {
        context.detail(&format!(
            "credentials saved to {}",
            context.credentials_path.display()
        ));
    }

    // The point of signing in is to use the account, so say what is on it.
    let user = client.user().await?;
    for entry in &user.child_list {
        let name = entry.nickname.as_deref().unwrap_or("(no nickname)");
        println!("{}\t{name}", entry.cid);
    }
    if context.config.child().is_none()
        && let [only] = user.child_list.as_slice()
    {
        let mut config = context.config.clone();
        config.set("child", &only.cid)?;
        crate::config::save(&context.config_path, &config)?;
        context.report(&format!(
            "One child on the account, so saved child = {}",
            only.cid
        ));
    }
    Ok(())
}

/// Says whether there is a session, and how long it has left.
fn status(context: &Context) -> Result<()> {
    let resolved = context.credentials()?;
    let theme = context.theme;

    match &resolved.email {
        Some(email) => println!("email\t{email}"),
        None => println!("email\t(none)"),
    }
    println!(
        "password\t{}",
        match (&resolved.password, resolved.password_from_environment) {
            (Some(_), true) => format!("set (from {PASSWORD_ENV})"),
            (Some(_), false) => "saved".to_owned(),
            (None, _) => "(none)".to_owned(),
        }
    );

    if let Some(session) = &resolved.session {
        let remaining = auth::seconds_remaining(session.expires_at, now_seconds());
        println!("session\tvalid");
        println!("expires_in\t{remaining}");
        eprintln!(
            "{}",
            theme.muted(&if remaining > 0 {
                format!(
                    "The session has {} left.",
                    format_duration(remaining as f64)
                )
            } else {
                "The session has expired; the next command will renew it.".to_owned()
            })
        );
    } else {
        println!("session\tnone");
        eprintln!("{}", theme.muted(&crate::session::not_signed_in_message()));
    }
    Ok(())
}

/// Forgets the session and the credentials.
fn logout(context: &Context) -> Result<()> {
    crate::credentials::remove(&context.credentials_path)?;
    context.report("Signed out. The saved session and password are gone.");
    context.detail(&format!("removed {}", context.credentials_path.display()));
    Ok(())
}

/// Which timezone to sign in with: the flag, then the setting, then a
/// question, then UTC.
fn resolve_timezone(context: &Context, flag: Option<String>) -> Result<String> {
    if let Some(named) = flag {
        crate::domain::Calendar::new(&named)?;
        return Ok(named);
    }
    if context.config.timezone != "UTC" {
        return Ok(context.config.timezone.clone());
    }
    // Only asked once, on the first sign-in, and it is the one setting that
    // makes every day boundary in the tool right or wrong.
    let question = Question::new(
        "timezone",
        "Which timezone does the family keep? (e.g. America/New_York)",
        "--timezone <ZONE>",
    )
    .with_default("UTC");
    let answer = prompt::ask(&question, context.theme)?;
    crate::domain::Calendar::new(&answer)?;
    Ok(answer)
}

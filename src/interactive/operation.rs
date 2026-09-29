//! Direct command execution and error recovery for menu selections.
use super::{
    catalog::CommandPath,
    draft::CommandDraft,
    pause,
    session::{SessionOptions, load_context},
    session_menu::choose,
};
use crate::{
    cli::{
        AuthAction, ChildAction, Command, ConfigAction, FeedAction, FoodsAction, NursingAction,
        SleepAction,
    },
    prompt,
    theme::Theme,
};
use anyhow::Result;

/// Returns true after a successful action that should return to home.
pub(super) async fn run(
    path: &CommandPath,
    globals: &SessionOptions,
    theme: Theme,
) -> Result<bool> {
    let draft = CommandDraft::new(path.clone());
    if draft
        .resolve(globals)?
        .command
        .as_ref()
        .is_some_and(is_read_view)
    {
        return run_view(&draft, globals, theme).await;
    }
    execute(&draft, globals, theme).await
}

async fn run_view(draft: &CommandDraft, globals: &SessionOptions, theme: Theme) -> Result<bool> {
    loop {
        let mut labels = vec!["Back".into()];
        match execute(draft, globals, theme).await {
            Ok(_) => {}
            Err(error) if prompt::is_cancelled(&error) => return Ok(false),
            Err(error) => {
                eprintln!("{}", theme.error_line("error:", &format!("{error:#}")));
                labels.push("Retry".into());
            }
        }
        match choose("Anything else?", &labels, 0, theme)? {
            1 => {}
            _ => return Ok(false),
        }
    }
}

// Only read-only views offer an explicit retry after failure.
const fn is_read_view(command: &Command) -> bool {
    matches!(
        command,
        Command::Now { .. }
            | Command::Dash { .. }
            | Command::Summary { .. }
            | Command::Trends { .. }
            | Command::Stripes { .. }
            | Command::Log { .. }
            | Command::Info
            | Command::Auth {
                action: AuthAction::Status
            }
            | Command::Child {
                action: ChildAction::List | ChildAction::Show
            }
            | Command::Config {
                action: ConfigAction::Show | ConfigAction::Path
            }
            | Command::Foods {
                action: FoodsAction::List { .. }
            }
            | Command::Sleep {
                action: SleepAction::Status
            }
            | Command::Feed {
                action: FeedAction::Nursing {
                    action: NursingAction::Status
                }
            }
    )
}

async fn execute(draft: &CommandDraft, globals: &SessionOptions, theme: Theme) -> Result<bool> {
    if draft.is_unchanged_edit() {
        eprintln!("The entry is unchanged.");
        pause(theme)?;
        return Ok(true);
    }
    let cli = draft.resolve(globals)?;
    let context = load_context(globals, theme)?;
    let command = cli
        .command
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no command selected"))?;
    crate::commands::dispatch(&context, command).await?;
    if is_read_view(command) {
        return Ok(false);
    }
    // Utilities keep their submenu; completed recording actions go home.
    let home = !matches!(
        command,
        Command::Foods { .. }
            | Command::Child { .. }
            | Command::Config { .. }
            | Command::Export { .. }
    );
    pause(theme)?;
    Ok(home)
}

#[cfg(test)]
mod tests {
    use super::is_read_view;
    use crate::cli::Cli;
    use clap::Parser;

    #[test]
    fn read_views_offer_retry() {
        for path in [
            "now",
            "dash",
            "summary",
            "trends",
            "stripes",
            "log",
            "info",
            "auth status",
            "child list",
            "child show",
            "config show",
            "config path",
            "foods list",
            "sleep status",
            "feed nursing status",
        ] {
            let cli =
                Cli::try_parse_from(std::iter::once("h").chain(path.split_whitespace())).unwrap();
            assert!(is_read_view(cli.command.as_ref().unwrap()), "{path}");
        }
    }

    #[test]
    fn writes_and_exports_do_not_offer_retry() {
        for path in [
            "diaper",
            "potty",
            "growth",
            "edit",
            "delete",
            "export",
            "feed bottle",
            "feed solids",
            "sleep start",
            "sleep stop",
            "sleep pause",
            "sleep resume",
            "sleep cancel",
            "sleep manual",
            "feed nursing start",
            "feed nursing stop",
            "feed nursing pause",
            "feed nursing resume",
            "feed nursing switch",
            "feed nursing cancel",
            "auth login",
            "auth logout",
            "child use",
            "config set",
            "foods add",
        ] {
            let cli =
                Cli::try_parse_from(std::iter::once("h").chain(path.split_whitespace())).unwrap();
            assert!(!is_read_view(cli.command.as_ref().unwrap()), "{path}");
        }
    }
}

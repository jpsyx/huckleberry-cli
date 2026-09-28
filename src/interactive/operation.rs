//! Run, options and error recovery for one pending command.
use super::{
    catalog::CommandPath,
    draft::CommandDraft,
    options, pause,
    session::{SessionOptions, load_context},
    session_menu::choose,
};
use crate::{cli::Command, prompt, theme::Theme};
use anyhow::Result;

/// Returns true after a successful action that should return to home.
pub(super) async fn run(
    path: &CommandPath,
    globals: &mut SessionOptions,
    theme: Theme,
) -> Result<bool> {
    let mut draft = CommandDraft::new(path.clone());
    loop {
        let index = choose(
            &format!("{}: ready", path.0.join(" ")),
            &["Run".into(), "Options".into(), "Back".into()],
            0,
            theme,
        )?;
        let result = match index {
            0 => execute(&draft, globals, theme).await.map(Some),
            1 => Box::pin(options::edit(theme, globals, &mut draft))
                .await
                .map(|()| None),
            _ => return Ok(false),
        };
        match result {
            Ok(Some(home)) => return Ok(home),
            Ok(None) => {}
            Err(error) if prompt::is_cancelled(&error) => return Ok(false),
            Err(error) => {
                eprintln!("{}", theme.error_line("error:", &format!("{error:#}")));
                pause(theme)?;
            }
        }
    }
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
    // Read views return to their submenu; completed recording actions go home.
    let home = !matches!(
        command,
        Command::Dash { .. }
            | Command::Log { .. }
            | Command::Now { .. }
            | Command::Summary { .. }
            | Command::Trends { .. }
            | Command::Stripes { .. }
            | Command::Info
            | Command::Foods { .. }
            | Command::Child { .. }
            | Command::Config { .. }
            | Command::Export { .. }
    );
    if !matches!(
        command,
        Command::Dash { .. } | Command::Log { search: None, .. }
    ) || !std::io::IsTerminal::is_terminal(&std::io::stdout())
    {
        pause(theme)?;
    }
    Ok(home)
}

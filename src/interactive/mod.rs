//! Persistent one-handed navigation over the existing typed command handlers.
pub mod catalog;
pub mod draft;
pub mod interrupt;
mod operation;
pub mod options;
pub mod session;
mod session_menu;
use crate::{cli::Cli, prompt, theme::Theme};
use anyhow::Result;
use catalog::{MenuId, MenuTarget};
use session::SessionOptions;
use session_menu::choose;

/// How the invocation is presented before opening configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryMode {
    /// Dispatch an explicitly named command.
    Command,
    /// Start a terminal session.
    Interactive,
    /// Print help without prompting.
    Help,
}

/// Pure entry decision; redirected stderr cannot host prompts.
#[must_use]
pub const fn entry_mode(
    has_command: bool,
    stdin_terminal: bool,
    stderr_terminal: bool,
) -> EntryMode {
    if has_command {
        EntryMode::Command
    } else if stdin_terminal && stderr_terminal {
        EntryMode::Interactive
    } else {
        EntryMode::Help
    }
}

/// Runs menus until Exit or cancellation at home.
pub async fn run(cli: &Cli, theme: Theme) -> Result<()> {
    let mut globals = SessionOptions::from_cli(cli);
    let mut stack = vec![(MenuId::Home, 0)];
    while let Some(&(menu, cursor)) = stack.last() {
        let entries = catalog::entries(menu);
        let mut labels = entries
            .iter()
            .map(|entry| entry.label.clone())
            .collect::<Vec<_>>();
        if menu != MenuId::Home {
            labels.push("Back".into());
        }
        let index = match choose(title(menu), &labels, cursor, theme) {
            Ok(index) => index,
            Err(error) if prompt::is_cancelled(&error) => {
                stack.pop();
                continue;
            }
            Err(error) => return Err(error),
        };
        if index == entries.len() {
            stack.pop();
            continue;
        }
        if let Some(frame) = stack.last_mut() {
            frame.1 = index;
        }
        let result = interrupt::run(Box::pin(visit(
            &entries[index].target,
            &mut globals,
            &mut stack,
            theme,
        )))
        .await;
        if let Err(error) = result {
            if !prompt::is_cancelled(&error) {
                eprintln!("{}", theme.error_line("error:", &format!("{error:#}")));
                let _ = pause(theme);
            }
        }
    }
    Ok(())
}

async fn visit(
    target: &MenuTarget,
    globals: &mut SessionOptions,
    stack: &mut Vec<(MenuId, usize)>,
    theme: Theme,
) -> Result<()> {
    match target {
        MenuTarget::Menu(menu) => stack.push((*menu, 0)),
        MenuTarget::Command(path) => {
            if Box::pin(operation::run(path, globals, theme)).await? {
                *stack = vec![(MenuId::Home, 0)];
            }
        }
        MenuTarget::SessionOptions => *globals = session::edit_options(globals, theme).await?,
        MenuTarget::Help => help(theme)?,
        MenuTarget::Version => {
            eprintln!("huckleberry-cli {}", env!("CARGO_PKG_VERSION"));
            pause(theme)?;
        }
        MenuTarget::Exit => stack.clear(),
    }
    Ok(())
}

pub(super) fn pause(theme: Theme) -> Result<()> {
    choose("Ready?", &["Continue".into()], 0, theme).map(|_| ())
}

fn help(theme: Theme) -> Result<()> {
    let mut paths = vec![catalog::CommandPath(vec![])];
    paths.extend(catalog::command_paths());
    let labels = paths
        .iter()
        .map(|path| {
            if path.0.is_empty() {
                "All commands".into()
            } else {
                path.0.join(" ")
            }
        })
        .collect::<Vec<_>>();
    let index = choose("Help for which command?", &labels, 0, theme)?;
    if let Some(mut metadata) = catalog::metadata(&paths[index]) {
        eprintln!("{}", metadata.render_long_help());
    }
    pause(theme)
}

const fn title(menu: MenuId) -> &'static str {
    match menu {
        MenuId::Home => "What would you like to do?",
        MenuId::Feed => "Log a feed",
        MenuId::Sleep => "Log sleep",
        MenuId::Nursing => "Nursing",
        MenuId::Visualizations => "Visualizations",
        MenuId::OtherLogging => "Other logging",
        MenuId::More => "More",
        MenuId::Foods => "Foods",
        MenuId::Children => "Children",
        MenuId::Account => "Account",
        MenuId::Settings => "Settings",
    }
}

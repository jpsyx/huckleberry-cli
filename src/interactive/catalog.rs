//! Human menu grouping over clap's canonical command tree.
use crate::cli::Cli;
use clap::CommandFactory;

/// Canonical command words, excluding the program name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPath(pub Vec<String>);

/// A submenu in the command tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuId {
    /// The everyday actions.
    Home,
    /// Feeding kinds.
    Feed,
    /// Sleep timer actions.
    Sleep,
    /// Nursing timer actions.
    Nursing,
    /// Read-only visualizations.
    Visualizations,
    /// Less frequent recordings.
    OtherLogging,
    /// Account and utilities.
    More,
    /// Food definitions.
    Foods,
    /// Child selection and profile.
    Children,
    /// Authentication.
    Account,
    /// Persistent settings.
    Settings,
}

/// Destination of one highlighted entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    /// Open another menu.
    Menu(MenuId),
    /// Configure or run a typed command.
    Command(CommandPath),
    /// Edit this session's overrides.
    SessionOptions,
    /// Read command-specific help.
    Help,
    /// Display the build version.
    Version,
    /// Leave the session.
    Exit,
}

/// A label and its destination.
#[derive(Debug, Clone)]
pub struct MenuEntry {
    /// Parent-facing name.
    pub label: String,
    /// What Enter opens.
    pub target: MenuTarget,
}

fn command(label: &str, path: &str) -> MenuEntry {
    MenuEntry {
        label: label.into(),
        target: MenuTarget::Command(CommandPath(
            path.split_whitespace().map(str::to_owned).collect(),
        )),
    }
}
fn menu(label: &str, id: MenuId) -> MenuEntry {
    MenuEntry {
        label: label.into(),
        target: MenuTarget::Menu(id),
    }
}

/// Entries in the approved home order and its submenus.
#[must_use]
pub fn entries(id: MenuId) -> Vec<MenuEntry> {
    match id {
        MenuId::Home => vec![
            command("Log a diaper", "diaper"),
            menu("Log a feed", MenuId::Feed),
            menu("Log sleep", MenuId::Sleep),
            command("Edit", "edit"),
            command("Delete", "delete"),
            menu("Visualizations", MenuId::Visualizations),
            command("View logs", "log"),
            menu("Other logging", MenuId::OtherLogging),
            menu("More", MenuId::More),
            MenuEntry {
                label: "Exit".into(),
                target: MenuTarget::Exit,
            },
        ],
        MenuId::Visualizations => vec![
            command("Dashboard", "dash"),
            command("Trends", "trends"),
            command("Summary", "summary"),
            command("Sleep stripes", "stripes"),
            command("Current status", "now"),
        ],
        MenuId::OtherLogging => vec![command("Potty", "potty"), command("Growth", "growth")],
        MenuId::More => more(),
        MenuId::Feed => vec![
            command("Bottle", "feed bottle"),
            menu("Nursing", MenuId::Nursing),
            command("Solids", "feed solids"),
        ],
        MenuId::Sleep => children("sleep"),
        MenuId::Nursing => children("feed nursing"),
        MenuId::Foods => children("foods"),
        MenuId::Children => children("child"),
        MenuId::Account => children("auth"),
        MenuId::Settings => children("config"),
    }
}

fn more() -> Vec<MenuEntry> {
    let mut entries = vec![
        menu("Foods", MenuId::Foods),
        menu("Children", MenuId::Children),
        menu("Account", MenuId::Account),
        menu("Settings", MenuId::Settings),
        command("Export", "export"),
        command("About this build", "info"),
    ];
    entries.extend([
        MenuEntry {
            label: "Session options".into(),
            target: MenuTarget::SessionOptions,
        },
        MenuEntry {
            label: "Help".into(),
            target: MenuTarget::Help,
        },
        MenuEntry {
            label: "Version".into(),
            target: MenuTarget::Version,
        },
    ]);
    entries
}

fn children(parent: &str) -> Vec<MenuEntry> {
    let path = CommandPath(parent.split_whitespace().map(str::to_owned).collect());
    metadata(&path).map_or_else(Vec::new, |command_meta| {
        command_meta
            .get_subcommands()
            .filter(|child| child.get_name() != "help")
            .map(|child| {
                command(
                    &crate::render::output::words(child.get_name()),
                    &format!("{parent} {}", child.get_name()),
                )
            })
            .collect()
    })
}

/// Finds clap metadata, including global arguments and visible aliases.
#[must_use]
pub fn metadata(path: &CommandPath) -> Option<clap::Command> {
    let mut root = Cli::command();
    root.build();
    let mut current = &root;
    for word in &path.0 {
        current = current.get_subcommands().find(|child| {
            child.get_name() == word || child.get_all_aliases().any(|alias| alias == word)
        })?;
    }
    Some(current.clone())
}

/// Every executable route, obtained by walking the same menus the parent uses.
#[must_use]
pub fn command_paths() -> Vec<CommandPath> {
    let mut queue = vec![MenuId::Home];
    let mut paths = Vec::new();
    while let Some(id) = queue.pop() {
        for entry in entries(id) {
            match entry.target {
                MenuTarget::Menu(child) => queue.push(child),
                MenuTarget::Command(path) => paths.push(path),
                _ => {}
            }
        }
    }
    paths
}

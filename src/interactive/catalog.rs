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

impl MenuId {
    /// Every menu, so a test can walk the whole tree without knowing it.
    pub const ALL: [Self; 11] = [
        Self::Home,
        Self::Feed,
        Self::Sleep,
        Self::Nursing,
        Self::Visualizations,
        Self::OtherLogging,
        Self::More,
        Self::Foods,
        Self::Children,
        Self::Account,
        Self::Settings,
    ];
}

/// The heading this menu is asking its question under.
#[must_use]
pub const fn title(id: MenuId) -> &'static str {
    match id {
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

/// Destination of one highlighted entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuTarget {
    /// Open another menu.
    Menu(MenuId),
    /// Run a typed command.
    Command(CommandPath),
    /// Return to the menu this one was opened from.
    Back,
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
///
/// Every submenu ends in Back and Home ends in Exit, so the way out is always
/// a row on the screen as well as a key: one hand should never have to
/// remember an escape.
#[must_use]
pub fn entries(id: MenuId) -> Vec<MenuEntry> {
    let mut entries = rows(id);
    if id != MenuId::Home {
        entries.push(MenuEntry {
            label: "Back".into(),
            target: MenuTarget::Back,
        });
    }
    entries
}

fn rows(id: MenuId) -> Vec<MenuEntry> {
    match id {
        MenuId::Home => vec![
            command("View latest", "now"),
            command("Log a diaper", "diaper"),
            menu("Log a feed", MenuId::Feed),
            menu("Log sleep", MenuId::Sleep),
            command("Edit", "edit"),
            menu("Visualizations", MenuId::Visualizations),
            command("View logs", "log"),
            menu("Other logging", MenuId::OtherLogging),
            command("Delete", "delete"),
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

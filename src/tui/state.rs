//! What the shell is showing, and what a motion does to it.
//!
//! Everything here is pure. The loop in [`super::run`] owns the terminal, the
//! network and the command handlers; this owns the decisions, which is what
//! makes "does `h` at Home end the session" a test rather than something a
//! parent finds out at 3am.

use super::facts::Facts;
use super::keys::Motion;
use crate::interactive::catalog::{self, CommandPath, MenuEntry, MenuId, MenuTarget};

/// Whether this command is the dashboard.
#[must_use]
pub fn is_dashboard(path: &CommandPath) -> bool {
    path.0.first().is_some_and(|word| word == "dash")
}

/// One menu on the stack: which one, where the cursor is, and how far it has
/// scrolled. Each level keeps its own cursor so backing out of a submenu lands
/// on the row it was opened from.
struct Level {
    menu: MenuId,
    name: String,
    cursor: usize,
    top: usize,
}

impl Level {
    const fn new(menu: MenuId, name: String) -> Self {
        Self {
            menu,
            name,
            cursor: 0,
            top: 0,
        }
    }
}

/// One row, ready to draw.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// What the parent reads.
    pub label: String,
    /// True when opening this row reveals another menu rather than doing
    /// something. The screen marks those, so nothing is a surprise.
    pub opens_menu: bool,
}

/// What the shell has to do that the state cannot do for itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Intent {
    /// Redraw and wait for the next key.
    Stay,
    /// Leave the full screen and run this command.
    Run(CommandPath),
    /// Leave the full screen and show command help.
    Help,
    /// Re-read everything the widgets show.
    Refresh,
    /// Open the dashboard in the panel where the menu is.
    Dashboard,
    /// Report the build version.
    Version,
    /// End the session.
    Quit,
}

/// Everything the shell is holding.
pub struct App {
    levels: Vec<Level>,
    status: Option<String>,
    /// What the widgets draw, and how stale it is.
    pub facts: Facts,
    /// The dashboard, when it is the thing in the panel.
    pub dashboard: Option<crate::dashboard::State>,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    /// Opens on Home, with the first row under the cursor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            levels: vec![Level::new(MenuId::Home, "Home".into())],
            status: None,
            facts: Facts::new(),
            dashboard: None,
        }
    }

    /// The menu currently on screen.
    #[must_use]
    pub fn menu(&self) -> MenuId {
        self.level().menu
    }

    /// The heading above the rows.
    #[must_use]
    pub fn title(&self) -> &'static str {
        catalog::title(self.menu())
    }

    /// Where this menu sits, for example `Home › Log a feed`.
    #[must_use]
    pub fn breadcrumb(&self) -> String {
        self.levels
            .iter()
            .map(|level| level.name.as_str())
            .collect::<Vec<_>>()
            .join(" › ")
    }

    /// The rows on screen, in order.
    #[must_use]
    pub fn rows(&self) -> Vec<Row> {
        catalog::entries(self.menu())
            .into_iter()
            .map(|entry| Row {
                opens_menu: matches!(entry.target, MenuTarget::Menu(_)),
                label: entry.label,
            })
            .collect()
    }

    /// Which row is highlighted.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.level().cursor
    }

    /// The first visible row, once the menu is taller than its viewport.
    #[must_use]
    pub fn top(&self) -> usize {
        self.level().top
    }

    /// The transient line at the foot of the screen, if there is one.
    #[must_use]
    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    /// Puts one line at the foot of the screen until the next keystroke.
    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
    }

    /// Returns to Home, which is where a finished recording belongs.
    pub fn go_home(&mut self) {
        self.levels.truncate(1);
    }

    /// Opens the dashboard on what has already been read.
    ///
    /// It needs no read of its own: the shell has one, and refreshing the
    /// widgets refreshes this too. Before the first read there is nothing to
    /// draw, so it says so rather than opening empty.
    pub fn open_dashboard(&mut self, days: usize) {
        let Some(reading) = self.facts.reading() else {
            self.set_status("nothing read yet · press r");
            return;
        };
        self.dashboard = Some(
            crate::dashboard::State::new(reading.dataset.clone(), reading.calendar.clone(), days)
                .with_rule(reading.rule)
                .in_panel(),
        );
    }

    /// Takes on a fresh read, and gives it to the dashboard as well.
    pub fn refreshed(
        &mut self,
        dataset: crate::domain::types::Dataset,
        calendar: crate::domain::Calendar,
        units: crate::cli::Units,
        rule: crate::domain::today::DayRule,
    ) {
        if let Some(dashboard) = self.dashboard.as_mut() {
            dashboard.calendar = calendar.clone();
            dashboard.rule = rule;
            dashboard.replace(dataset.clone());
        }
        self.facts.replace(dataset, calendar, units, rule);
    }

    /// Closes the dashboard, which puts the menu back.
    ///
    /// Returns whether there was one, so a key that closes it is not also a
    /// key that does something to the menu underneath.
    pub fn close_dashboard(&mut self) -> bool {
        self.dashboard.take().is_some()
    }

    /// Applies one motion, given how many rows the viewport can show.
    pub fn apply(&mut self, motion: Motion, height: usize) -> Intent {
        self.status = None;
        let entries = catalog::entries(self.menu());
        let last = entries.len().saturating_sub(1);
        match motion {
            Motion::Ignore => return Intent::Stay,
            Motion::Next => self.level_mut().cursor = self.cursor().saturating_add(1).min(last),
            Motion::Previous => self.level_mut().cursor = self.cursor().saturating_sub(1),
            Motion::First => self.level_mut().cursor = 0,
            Motion::Last => self.level_mut().cursor = last,
            Motion::Highlight(index) => {
                if index >= entries.len() {
                    return Intent::Stay;
                }
                self.level_mut().cursor = index;
            }
            Motion::Back => self.pop(),
            // Cancel what you are in. At the top level that is the session.
            Motion::Cancel => {
                if self.levels.len() == 1 {
                    return Intent::Quit;
                }
                self.pop();
            }
            Motion::Quit => return Intent::Quit,
            // Deliberately not a movement: refreshing under somebody's cursor
            // and then moving it would be its own small betrayal.
            Motion::Refresh => return Intent::Refresh,
            Motion::Open => {
                let Some(entry) = entries.get(self.cursor()) else {
                    return Intent::Stay;
                };
                return self.open(entry);
            }
        }
        self.scroll(height, entries.len());
        Intent::Stay
    }

    /// Opens whatever the highlighted row points at.
    fn open(&mut self, entry: &MenuEntry) -> Intent {
        match &entry.target {
            MenuTarget::Menu(menu) => {
                self.levels.push(Level::new(*menu, entry.label.clone()));
                Intent::Stay
            }
            // The dashboard is a screen rather than a command with
            // questions, so the shell draws it itself. It stays a command in
            // the catalog, because `h dash` is still one.
            MenuTarget::Command(path) if is_dashboard(path) => Intent::Dashboard,
            MenuTarget::Command(path) => Intent::Run(path.clone()),
            MenuTarget::Back => {
                self.pop();
                Intent::Stay
            }
            MenuTarget::Help => Intent::Help,
            MenuTarget::Version => Intent::Version,
        }
    }

    /// Back at Home is a no-op: see [`super::keys::motion_for`].
    fn pop(&mut self) {
        if self.levels.len() > 1 {
            self.levels.pop();
        }
    }

    fn scroll(&mut self, height: usize, count: usize) {
        let (top, cursor) = (self.top(), self.cursor());
        self.level_mut().top = crate::listing::state::scrolled(top, cursor, height, count);
    }

    fn level(&self) -> &Level {
        // The stack is created with Home and `pop` never empties it.
        self.levels.last().expect("the menu stack is never empty")
    }

    fn level_mut(&mut self) -> &mut Level {
        self.levels
            .last_mut()
            .expect("the menu stack is never empty")
    }
}

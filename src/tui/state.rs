//! What the shell is showing, and what a motion does to it.
//!
//! Everything here is pure. The loop in [`super::run`] owns the terminal, the
//! network and the command handlers; this owns the decisions, which is what
//! makes "does `h` at Home end the session" a test rather than something a
//! parent finds out at 3am.

use super::keys::Motion;
use crate::interactive::catalog::{self, CommandPath, MenuEntry, MenuId, MenuTarget};

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
    /// Report the build version.
    Version,
    /// End the session.
    Quit,
}

/// Everything the shell is holding.
pub struct App {
    levels: Vec<Level>,
    status: Option<String>,
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
            Motion::Quit => return Intent::Quit,
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
            MenuTarget::Command(path) => Intent::Run(path.clone()),
            MenuTarget::Back => {
                self.pop();
                Intent::Stay
            }
            MenuTarget::Help => Intent::Help,
            MenuTarget::Version => Intent::Version,
            MenuTarget::Exit => Intent::Quit,
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

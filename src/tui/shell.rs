//! The alternate screen, and stepping out of it.
//!
//! This is the only file in `tui/` that touches a terminal, and it holds no
//! decision worth asserting.
//!
//! It draws on **stderr**, not stdout. The menu is the conversation and the
//! commands it runs are the data, so `h > entries.txt` still fills the file
//! with what the commands printed rather than with escape sequences.
//!
//! [`Shell::suspend`] is what lets the shell keep every existing command
//! without rewriting one: the full screen steps aside, the handler asks its
//! questions and prints its receipt exactly as it does from the command line,
//! and the menu comes back afterwards. Panels replace those one at a time
//! later; nothing has to wait for them.

use anyhow::{Context, Result};
use crossterm::{cursor, execute, terminal};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{Stderr, stderr};
use std::sync::Once;

use super::keys::{Motion, motion_for};
use super::state::App;

/// The terminal this shell draws on.
type Screen = Terminal<CrosstermBackend<Stderr>>;

/// Owns the full screen for as long as the session lasts.
pub struct Shell {
    screen: Screen,
    showing: bool,
}

impl Shell {
    /// Takes the screen, installing the panic hook exactly once.
    pub fn enter() -> Result<Self> {
        install_panic_hook();
        take_screen()?;
        Ok(Self {
            screen: open_screen()?,
            showing: true,
        })
    }

    /// Draws one frame, and returns how many rows the menu can show.
    pub fn draw(&mut self, app: &App) -> Result<usize> {
        let frame = self
            .screen
            .draw(|frame| super::draw(frame, app))
            .context("drawing the menu")?;
        Ok(super::draw::viewport(frame.area.height))
    }

    /// Waits for a keystroke. A resize (or anything else) returns nothing,
    /// which the loop reads as "draw again".
    pub fn next_motion(&mut self) -> Result<Option<Motion>> {
        match crossterm::event::read().context("reading a keystroke")? {
            crossterm::event::Event::Key(key) => Ok(Some(motion_for(key))),
            _ => Ok(None),
        }
    }

    /// Gives the terminal back so a command can ask its questions and print
    /// its receipt where the parent can scroll back to them.
    pub fn suspend(&mut self) -> Result<()> {
        if self.showing {
            self.showing = false;
            release_screen()?;
        }
        Ok(())
    }

    /// Takes the screen again, redrawing all of it: whatever the command
    /// printed is still on the ordinary screen underneath.
    ///
    /// The terminal is rebuilt rather than cleared. A new one starts with an
    /// empty buffer, so its first draw paints every cell of the blank screen
    /// that was just entered; `Terminal::clear` would instead ask the terminal
    /// where its cursor is and wait on stdin for the answer, which is the same
    /// stdin the menu reads its keys from.
    pub fn resume(&mut self) -> Result<()> {
        if !self.showing {
            take_screen()?;
            self.screen = open_screen()?;
            self.showing = true;
        }
        Ok(())
    }

    /// Hands the terminal back for good.
    pub fn leave(&mut self) -> Result<()> {
        self.suspend()
    }
}

impl Drop for Shell {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

/// A terminal over stderr, with nothing drawn on it yet.
fn open_screen() -> Result<Screen> {
    Terminal::new(CrosstermBackend::new(stderr())).context("opening the full-screen menu")
}

/// Raw mode and the alternate screen, on stderr.
fn take_screen() -> Result<()> {
    terminal::enable_raw_mode().context("enabling keyboard input")?;
    execute!(stderr(), terminal::EnterAlternateScreen, cursor::Hide)
        .context("opening the full-screen menu")
}

/// The inverse, in the order that leaves a usable shell behind: raw mode first,
/// because it has the wider effect.
fn release_screen() -> Result<()> {
    let raw = terminal::disable_raw_mode();
    let screen = execute!(stderr(), terminal::LeaveAlternateScreen, cursor::Show);
    raw.context("restoring terminal input")?;
    screen.context("restoring the terminal")
}

/// Restores the terminal before a panic prints, so a crash does not leave
/// somebody typing blind into a raw-mode shell.
///
/// Installed once for the process: `ratatui::init` wraps the hook on every
/// call, and this session enters and leaves the full screen once per command.
fn install_panic_hook() {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = release_screen();
            previous(info);
        }));
    });
}

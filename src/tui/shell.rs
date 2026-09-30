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

use core::time::Duration;

use super::keys::{Motion, motion_for};
use super::state::App;
use crate::prompt::host::Input;

/// How long the loop waits for a keystroke before drawing again.
///
/// The widgets count up: "36m ago" becomes "37m ago" and a running sleep ticks
/// whether or not anybody touches the keyboard, so the screen redraws on its
/// own. Once a second is enough for a clock and cheap enough to be invisible.
pub const TICK: Duration = Duration::from_secs(1);

/// How long it waits while a command is running.
///
/// A command's question arrives between keystrokes rather than because of one,
/// and a second between asking and appearing reads as the program having
/// stopped. This is short enough to be invisible and long enough not to spin.
pub const WORKING: Duration = Duration::from_millis(30);

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

    /// Draws one frame as of `at`, and returns how many rows the menu shows.
    pub fn draw(&mut self, app: &App, at: f64) -> Result<usize> {
        self.draw_with(app, None, at)
    }

    /// Draws one frame, with a command running where the menu was.
    pub fn draw_with(
        &mut self,
        app: &App,
        job: Option<&super::job::Job>,
        at: f64,
    ) -> Result<usize> {
        let frame = self
            .screen
            .draw(|frame| super::draw::draw_with(frame, app, job, at))
            .context("drawing the menu")?;
        Ok(super::draw::viewport(frame.area, app))
    }

    /// Waits a tick for a keystroke. Nothing (a timeout, a resize, anything
    /// else) means "draw again", which is what keeps the clocks moving.
    pub fn next_motion(&mut self) -> Result<Option<Motion>> {
        Ok(match self.next_input(TICK)? {
            Some(Input::Key(key)) => Some(motion_for(key)),
            _ => None,
        })
    }

    /// The same wait, with whatever arrived, for passing to a running command.
    ///
    /// A paste comes back whole rather than as one key per character, because
    /// bracketed paste is on: dictation and clipboard tools deliver a burst,
    /// and a burst is easier to keep together than to put back together.
    pub fn next_input(&mut self, wait: Duration) -> Result<Option<Input>> {
        if !crossterm::event::poll(wait).context("waiting for a keystroke")? {
            return Ok(None);
        }
        match crossterm::event::read().context("reading a keystroke")? {
            crossterm::event::Event::Key(key) => Ok(Some(Input::Key(key))),
            crossterm::event::Event::Paste(text) => Ok(Some(Input::Pasted(text))),
            _ => Ok(None),
        }
    }

    /// Gives the terminal back.
    ///
    /// Nothing steps aside any more: every command runs in the panel where the
    /// menu was. This is how the shell ends, and [`Self::leave`] is its only
    /// caller.
    fn suspend(&mut self) -> Result<()> {
        if self.showing {
            self.showing = false;
            release_screen()?;
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
    execute!(
        stderr(),
        terminal::EnterAlternateScreen,
        crossterm::event::EnableBracketedPaste,
        cursor::Hide
    )
    .context("opening the full-screen menu")
}

/// The inverse, in the order that leaves a usable shell behind: raw mode first,
/// because it has the wider effect.
fn release_screen() -> Result<()> {
    let raw = terminal::disable_raw_mode();
    let screen = execute!(
        stderr(),
        crossterm::event::DisableBracketedPaste,
        terminal::LeaveAlternateScreen,
        cursor::Show
    );
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

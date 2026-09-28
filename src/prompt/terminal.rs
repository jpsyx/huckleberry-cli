//! Scoped terminal ownership shared by selection and text input.
use anyhow::{Context, Result};
use crossterm::event::{DisableBracketedPaste, EnableBracketedPaste};
use crossterm::{cursor, execute, terminal};

/// Restores raw mode and cursor on normal and error returns.
pub struct TerminalGuard {
    active: bool,
}

impl TerminalGuard {
    /// Enables raw input without entering an alternate screen.
    pub fn enter() -> Result<Self> {
        terminal::enable_raw_mode().context("enabling keyboard input")?;
        let guard = Self { active: true };
        execute!(std::io::stderr(), EnableBracketedPaste).context("enabling pasted input")?;
        Ok(guard)
    }

    /// Explicit restoration, with errors available to the caller.
    pub fn restore(&mut self) -> Result<()> {
        if self.active {
            self.active = false;
            let shown = execute!(std::io::stderr(), cursor::Show, DisableBracketedPaste);
            let restored = terminal::disable_raw_mode();
            shown.context("restoring cursor")?;
            restored.context("restoring terminal input")?;
        }
        Ok(())
    }
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.restore();
    }
}

//! Thin menu input loop on stderr.
use super::{MenuItem, Selection, SelectionAction, render};
use crate::{
    prompt::{Cancelled, terminal::TerminalGuard},
    theme::Theme,
};
use anyhow::{Context, Result, bail};
use crossterm::{cursor, event, execute, terminal};
use std::io::{Write, stderr};

/// Chooses a numbered entry; no value is returned on cancellation.
pub fn choose(label: &str, items: &[MenuItem], default: usize, theme: Theme) -> Result<usize> {
    if items.is_empty() {
        bail!("there are no choices available");
    }
    if crate::prompt::host::hosted() {
        return hosted(label, items, default, theme);
    }
    if !crate::prompt::available() {
        bail!("interactive selection needs terminal input and stderr");
    }
    let mut guard = TerminalGuard::enter()?;
    let result = collect(label, items, default, theme);
    guard.restore()?;
    result
}

/// The same menu, drawn by whatever is hosting the screen.
///
/// The same state and the same renderer: only who owns the terminal differs.
fn hosted(label: &str, items: &[MenuItem], default: usize, theme: Theme) -> Result<usize> {
    let mut state = Selection::new(items.len(), default);
    let (width, height) = crate::prompt::host::size();
    loop {
        let lines = render(label, items, &state, width, height, theme);
        let key = crate::prompt::host::frame(lines)?;
        match state.apply(key, items.len(), usize::from(height).saturating_sub(2)) {
            SelectionAction::Stay => {}
            SelectionAction::Submit(index) => return Ok(index),
            SelectionAction::Cancel => return Err(Cancelled.into()),
        }
    }
}

fn collect(label: &str, items: &[MenuItem], default: usize, theme: Theme) -> Result<usize> {
    let mut state = Selection::new(items.len(), default);
    let mut output = stderr();
    let mut drawn = 0;
    execute!(output, cursor::Hide)?;
    loop {
        let (width, height) = terminal::size().unwrap_or((80, 24));
        let lines = render(label, items, &state, width, height.saturating_sub(1), theme);
        redraw(&mut output, &lines, drawn)?;
        drawn = lines.len();
        if let event::Event::Key(key) = event::read().context("reading menu selection")? {
            match state.apply(key, items.len(), usize::from(height).saturating_sub(3)) {
                SelectionAction::Stay => {}
                SelectionAction::Submit(index) => {
                    redraw(
                        &mut output,
                        &[super::clip(
                            &format!("{label} {}", items[index].label),
                            usize::from(width).saturating_sub(1),
                        )],
                        drawn,
                    )?;
                    return Ok(index);
                }
                SelectionAction::Cancel => {
                    redraw(&mut output, &[], drawn)?;
                    return Err(Cancelled.into());
                }
            }
        }
    }
}

fn redraw(output: &mut impl Write, lines: &[String], previous: usize) -> Result<()> {
    if previous > 0 {
        execute!(output, cursor::MoveUp(previous as u16))?;
    }
    execute!(
        output,
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::FromCursorDown)
    )?;
    for line in lines {
        write!(output, "{line}\r\n")?;
    }
    output.flush().context("drawing menu")
}

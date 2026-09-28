//! Raw-mode text input with cancellable, masked and pasted answers.
use super::{Cancelled, Question, terminal::TerminalGuard};
use crate::theme::Theme;
use anyhow::{Context, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute, terminal,
};
use std::io::{Write, stderr};

/// Reads one line, preserving the question's default as an Enter action.
pub(super) fn read(question: &Question<'_>, theme: Theme, secret: bool) -> Result<String> {
    let mut guard = TerminalGuard::enter()?;
    let result = collect(question, theme, secret);
    let restored = guard.restore();
    eprintln!();
    restored?;
    result
}

fn collect(question: &Question<'_>, theme: Theme, secret: bool) -> Result<String> {
    let mut output = stderr();
    write!(output, "{}\r\n", theme.prompt(question.label))?;
    if let Some(help) = question.help {
        write!(output, "{}\r\n", theme.muted(help))?;
    }
    let mut typed = String::new();
    loop {
        draw(&mut output, question, &typed, secret)?;
        match event::read().context("reading text")? {
            Event::Paste(text) => {
                typed.extend(text.chars().filter(|character| !character.is_control()));
            }
            Event::Key(key) if key.kind == KeyEventKind::Press => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('c' | 'd') => return Err(Cancelled.into()),
                        KeyCode::Char('u') => typed.clear(),
                        _ => {}
                    }
                } else {
                    match key.code {
                        KeyCode::Enter => return Ok(typed),
                        KeyCode::Esc => return Err(Cancelled.into()),
                        KeyCode::Backspace => {
                            typed.pop();
                        }
                        KeyCode::Char(character) => typed.push(character),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn draw(output: &mut impl Write, question: &Question<'_>, typed: &str, secret: bool) -> Result<()> {
    let prefix = question
        .default
        .filter(|_| !secret)
        .map_or_else(|| "> ".into(), |default| format!("[{default}] > "));
    let visible = if secret {
        "*".repeat(typed.chars().count())
    } else {
        typed.to_owned()
    };
    let width = usize::from(terminal::size().unwrap_or((80, 24)).0).saturating_sub(1);
    let line = super::select::clip(&format!("{prefix}{visible}"), width);
    execute!(
        output,
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::CurrentLine),
        cursor::Show
    )?;
    write!(output, "{line}")?;
    output.flush().context("drawing input")
}

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
    if crate::prompt::host::hosted() {
        return hosted(question, theme, secret);
    }
    let mut guard = TerminalGuard::enter()?;
    let result = collect(question, theme, secret);
    let restored = guard.restore();
    eprintln!();
    restored?;
    result
}

/// The same reader, drawn by whatever is hosting the screen.
fn hosted(question: &Question<'_>, theme: Theme, secret: bool) -> Result<String> {
    let mut typed = String::new();
    loop {
        let mut lines = vec![theme.prompt(question.label)];
        if let Some(help) = question.help {
            lines.push(theme.muted(help));
        }
        lines.push(theme.value(&typed_line(question, &typed, secret)));
        match crate::prompt::host::frame(lines)? {
            crate::prompt::host::Input::Pasted(text) => paste(&mut typed, &text),
            crate::prompt::host::Input::Key(key) => match typing(key, &mut typed) {
                Typing::Stay => {}
                Typing::Done => return Ok(typed),
                Typing::Cancel => return Err(Cancelled.into()),
            },
        }
    }
}

/// What the prompt line reads while somebody types into it.
fn typed_line(question: &Question<'_>, typed: &str, secret: bool) -> String {
    let prefix = question
        .default
        .filter(|_| !secret)
        .map_or_else(|| "> ".into(), |default| format!("[{default}] > "));
    let visible = if secret {
        "*".repeat(typed.chars().count())
    } else {
        typed.to_owned()
    };
    format!("{prefix}{visible}")
}

/// What a paste adds to what has been typed.
///
/// Control characters are left out rather than obeyed: a pasted newline would
/// otherwise read as Enter and submit the answer halfway through, which is how
/// a dictated phrase loses everything after its first line.
fn paste(typed: &mut String, text: &str) {
    typed.extend(text.chars().filter(|character| !character.is_control()));
}

/// What one key does to what has been typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Typing {
    /// Redraw and wait.
    Stay,
    /// Take what is there.
    Done,
    /// Abandon the answer.
    Cancel,
}

fn typing(key: crossterm::event::KeyEvent, typed: &mut String) -> Typing {
    if key.kind != KeyEventKind::Press {
        return Typing::Stay;
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c' | 'd') => Typing::Cancel,
            KeyCode::Char('u') => {
                typed.clear();
                Typing::Stay
            }
            _ => Typing::Stay,
        };
    }
    match key.code {
        KeyCode::Enter => Typing::Done,
        KeyCode::Esc => Typing::Cancel,
        KeyCode::Backspace => {
            typed.pop();
            Typing::Stay
        }
        KeyCode::Char(character) => {
            typed.push(character);
            Typing::Stay
        }
        _ => Typing::Stay,
    }
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
    let line = preview(&prefix, &visible, width);
    execute!(
        output,
        cursor::MoveToColumn(0),
        terminal::Clear(terminal::ClearType::CurrentLine),
        cursor::Show
    )?;
    write!(output, "{line}")?;
    output.flush().context("drawing input")
}

fn preview(prefix: &str, value: &str, width: usize) -> String {
    let prefix = super::select::clip(prefix, if value.is_empty() { width } else { width / 3 });
    let available = width.saturating_sub(ratatui::text::Line::raw(&prefix).width());
    let mut tail = String::new();
    for character in value
        .chars()
        .rev()
        .filter(|character| !character.is_control())
    {
        let candidate = format!("{character}{tail}");
        if ratatui::text::Line::raw(&candidate).width() > available {
            break;
        }
        tail = candidate;
    }
    format!("{prefix}{tail}")
}

#[cfg(test)]
mod tests {
    #[test]
    fn long_input_keeps_the_insertion_point_visible() {
        let line = super::preview(
            "[a very long default] > ",
            "beginning of a long note END",
            18,
        );
        assert!(line.ends_with("END"));
        assert!(ratatui::text::Line::raw(line).width() <= 18);
    }
}

#[cfg(test)]
mod pasting {
    use super::*;

    /// A dictated phrase arrives as one paste, and must land whole.
    #[test]
    fn a_paste_lands_whole_and_after_whatever_was_already_typed() {
        let mut typed = "at ".to_owned();
        paste(&mut typed, "30 minutes ago");
        assert_eq!(typed, "at 30 minutes ago");
    }

    /// A pasted newline would otherwise read as Enter and submit the answer
    /// halfway through, which is how a two-line dictation loses its second
    /// line.
    #[test]
    fn control_characters_in_a_paste_are_left_out_rather_than_obeyed() {
        let mut typed = String::new();
        paste(&mut typed, "30 minutes\nago\t");
        assert_eq!(typed, "30 minutesago");
    }
}

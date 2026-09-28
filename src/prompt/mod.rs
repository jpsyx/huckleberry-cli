//! Shared prompts: finite choices are menus, free values use a text editor.
mod question;
mod secret;
pub mod select;
pub mod terminal;
mod text;
pub mod time;

use crate::theme::Theme;
use anyhow::{Result, bail};
pub use question::{Choice, Question, Reply, interpret, render, unanswerable_message};
pub use secret::{Keystroke, MASK, interpret_keystroke};

/// A cancelled question, distinct from an I/O or validation failure.
#[derive(Debug)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("cancelled")
    }
}
impl std::error::Error for Cancelled {}

/// Recognizes cancellation even after contextual error wrapping.
#[must_use]
pub fn is_cancelled(error: &anyhow::Error) -> bool {
    error.is::<Cancelled>()
}

/// Whether both input and prompt output support interaction.
#[must_use]
pub fn available() -> bool {
    use std::io::IsTerminal;
    std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
}

/// Asks a required question; explicit-command callers retain flag errors on pipes.
pub fn ask(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !available() {
        bail!(unanswerable_message(question));
    }
    if !question.choices.is_empty() {
        return select::answer(question, theme)?.ok_or_else(|| Cancelled.into());
    }
    loop {
        let answer = text::read(question, theme, false)?;
        match interpret(&answer, &[], question.default) {
            Reply::Accepted(value) => return Ok(value),
            _ => eprintln!("{}", theme.warning("An answer is required.")),
        }
    }
}

/// Asks an optional value with a visible Skip entry.
pub fn ask_optional(question: &Question<'_>, theme: Theme) -> Result<Option<String>> {
    if !available() {
        return Ok(None);
    }
    select::answer(&question.optional(), theme)
}

/// Reads a secret without showing its contents.
pub fn ask_secret(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !available() {
        bail!(unanswerable_message(question));
    }
    let answer = text::read(question, theme, true)?;
    if answer.trim().is_empty() {
        bail!("no {}: nothing was typed", question.subject);
    }
    Ok(answer)
}

/// A Yes/No menu whose highlighted answer is the existing default.
pub fn confirm(label: &str, default: bool, theme: Theme) -> Result<bool> {
    if !available() {
        return Ok(default);
    }
    let choices = [
        Choice {
            value: "yes",
            hint: "Yes",
        },
        Choice {
            value: "no",
            hint: "No",
        },
    ];
    let question = Question::new("confirmation", label, "--yes")
        .with_choices(&choices)
        .with_default(if default { "yes" } else { "no" });
    Ok(ask(&question, theme)? == "yes")
}

/// Interprets legacy line confirmations for noninteractive callers.
#[must_use]
pub fn interpret_confirmation(input: &str, default: bool) -> bool {
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
    }
}

#[cfg(test)]
mod tests;

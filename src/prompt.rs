//! Asking for a value that was left out.
//!
//! This is the one place that knows how to complete a missing argument, so
//! every command asks the same way and every command stays fully drivable with
//! flags alone. A [`Question`] names the value, the flag that would have
//! answered it, and the answers on offer; [`ask`] is the only part that talks
//! to the terminal, and everything it decides ([`interpret`], [`render`],
//! [`unanswerable_message`]) is a pure function tested below.
//!
//! The rule the module exists to keep: ask only when there is somebody to ask.
//! With stdin redirected (a pipe, a CI job, an agent) the failure names the
//! flag instead of hanging on input that will never arrive. See
//! `docs/rules/cli-ux.md`.

use std::io::{IsTerminal, Write};

use anyhow::{Context, Result, bail};

use crate::theme::Theme;

/// One of the answers a question offers, with a line saying what it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Choice<'a> {
    /// The value that is stored or passed on when this answer is picked.
    pub value: &'a str,
    /// A few words about what picking it does.
    pub hint: &'a str,
}

/// A value a command needs and was not given.
#[derive(Debug, Clone, Copy)]
pub struct Question<'a> {
    /// What is missing, as the failure names it: "name to greet". It follows
    /// the word "no", so it reads as a noun phrase without an article.
    pub subject: &'a str,
    /// What to ask, as a person reads it: "Who should I greet?".
    pub label: &'a str,
    /// The flag or argument that answers this without being asked.
    pub flag: &'a str,
    /// The answers on offer. Empty means free text.
    pub choices: &'a [Choice<'a>],
    /// What an empty answer means, if anything.
    pub default: Option<&'a str>,
}

impl<'a> Question<'a> {
    /// A free-text question with no default.
    #[must_use]
    pub const fn new(subject: &'a str, label: &'a str, flag: &'a str) -> Self {
        Self {
            subject,
            label,
            flag,
            choices: &[],
            default: None,
        }
    }

    /// Offers a fixed set of answers, pickable by name or by number.
    #[must_use]
    pub const fn with_choices(mut self, choices: &'a [Choice<'a>]) -> Self {
        self.choices = choices;
        self
    }

    /// Gives an empty answer a meaning.
    #[must_use]
    pub const fn with_default(mut self, default: &'a str) -> Self {
        self.default = Some(default);
        self
    }
}

/// What a typed line means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    /// A usable answer.
    Accepted(String),
    /// Nothing was typed and there is no default, so the question stands.
    Blank,
    /// Something was typed that is not one of the choices.
    Unknown(String),
}

/// Reads one typed line. A number picks the nth choice (counting from one), a
/// name picks the choice it matches, and an empty line takes the default.
#[must_use]
pub fn interpret(input: &str, choices: &[Choice<'_>], default: Option<&str>) -> Reply {
    let answer = input.trim();
    if answer.is_empty() {
        return default.map_or(Reply::Blank, |value| Reply::Accepted(value.to_owned()));
    }
    if choices.is_empty() {
        return Reply::Accepted(answer.to_owned());
    }
    let picked = answer
        .parse::<usize>()
        .ok()
        .and_then(|position| choices.get(position.wrapping_sub(1)))
        .or_else(|| choices.iter().find(|choice| choice.value == answer));
    picked.map_or_else(
        || Reply::Unknown(answer.to_owned()),
        |choice| Reply::Accepted(choice.value.to_owned()),
    )
}

/// The question as it is written to stderr: the label, the numbered choices,
/// and the default in brackets.
#[must_use]
pub fn render(question: &Question<'_>, theme: Theme) -> String {
    let width = question
        .choices
        .iter()
        .map(|choice| choice.value.len())
        .max()
        .unwrap_or(0);
    let choices: Vec<String> = question
        .choices
        .iter()
        .enumerate()
        .map(|(position, choice)| {
            let number = position + 1;
            let value = choice.value;
            format!(
                "  {} {}  {}\n",
                theme.muted(&format!("{number})")),
                theme.accent(&format!("{value:width$}")),
                theme.muted(choice.hint)
            )
        })
        .collect();
    let answer_line = question.default.map_or_else(
        || "> ".to_owned(),
        |default| format!("[{}] > ", theme.value(default)),
    );
    format!(
        "{}\n{}{answer_line}",
        theme.prompt(question.label),
        choices.concat()
    )
}

/// The failure when the answer is missing and there is nobody to ask.
#[must_use]
pub fn unanswerable_message(question: &Question<'_>) -> String {
    format!(
        "no {}: pass {} (stdin is not a terminal, so I cannot ask)",
        question.subject, question.flag
    )
}

/// Asks the question and returns the answer, re-asking while the answer is
/// unusable. Without a terminal it does not ask at all: it fails with the flag
/// that would have answered.
pub fn ask(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!(unanswerable_message(question));
    }
    loop {
        write_question(question, theme)?;

        let mut line = String::new();
        let read = std::io::stdin()
            .read_line(&mut line)
            .context("reading the answer")?;
        if read == 0 {
            // End of input: the terminal went away mid-question.
            bail!(unanswerable_message(question));
        }
        match interpret(&line, question.choices, question.default) {
            Reply::Accepted(answer) => return Ok(answer),
            Reply::Blank => complain(theme, "an answer is required"),
            Reply::Unknown(answer) => {
                complain(theme, &format!("`{answer}` is not one of the choices"));
            }
        }
    }
}

/// Puts the question on stderr, flushed, so the cursor waits on the same line.
fn write_question(question: &Question<'_>, theme: Theme) -> Result<()> {
    let mut output = std::io::stderr();
    write!(output, "{}", render(question, theme)).context("writing the question")?;
    output.flush().context("writing the question")
}

/// Says why the answer was not usable, on stderr, before asking again.
fn complain(theme: Theme, message: &str) {
    eprintln!("  {}", theme.warning(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: [Choice<'static>; 2] = [
        Choice {
            value: "greeting",
            hint: "the opening word",
        },
        Choice {
            value: "verbose",
            hint: "more detail",
        },
    ];

    #[test]
    fn free_text_is_taken_as_typed_once_trimmed() {
        assert_eq!(
            interpret("  Ada \n", &[], None),
            Reply::Accepted("Ada".to_owned())
        );
    }

    #[test]
    fn an_empty_answer_takes_the_default() {
        assert_eq!(
            interpret("\n", &[], Some("Hello")),
            Reply::Accepted("Hello".to_owned())
        );
    }

    #[test]
    fn an_empty_answer_without_a_default_leaves_the_question_standing() {
        assert_eq!(interpret("   \n", &[], None), Reply::Blank);
    }

    #[test]
    fn a_choice_can_be_picked_by_number() {
        assert_eq!(
            interpret("2", &SETTINGS, None),
            Reply::Accepted("verbose".to_owned())
        );
    }

    #[test]
    fn a_choice_can_be_picked_by_name() {
        assert_eq!(
            interpret("greeting", &SETTINGS, None),
            Reply::Accepted("greeting".to_owned())
        );
    }

    #[test]
    fn a_number_outside_the_list_is_not_a_choice() {
        for answer in ["0", "3", "-1"] {
            assert_eq!(
                interpret(answer, &SETTINGS, None),
                Reply::Unknown(answer.to_owned()),
                "{answer} should not pick anything"
            );
        }
    }

    #[test]
    fn something_else_entirely_is_reported_back() {
        assert_eq!(
            interpret("verbos", &SETTINGS, None),
            Reply::Unknown("verbos".to_owned())
        );
    }

    #[test]
    fn the_rendered_question_numbers_every_choice_and_shows_the_default() {
        let question = Question::new("setting to change", "Which setting?", "<KEY>")
            .with_choices(&SETTINGS)
            .with_default("greeting");
        let text = render(&question, Theme::dark(false));
        assert!(text.starts_with("Which setting?\n"), "{text}");
        assert!(text.contains("1) greeting  the opening word"), "{text}");
        assert!(text.contains("2) verbose   more detail"), "{text}");
        assert!(text.ends_with("[greeting] > "), "{text}");
    }

    #[test]
    fn a_free_text_question_renders_a_bare_prompt() {
        let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
        let text = render(&question, Theme::dark(false));
        assert_eq!(text, "Who should I greet?\n> ");
    }

    #[test]
    fn the_unanswerable_failure_names_the_flag() {
        let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
        assert_eq!(
            unanswerable_message(&question),
            "no name to greet: pass --name <NAME> (stdin is not a terminal, so I cannot ask)"
        );
    }
}

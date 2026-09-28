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

use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

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

/// Asks for something that must not appear on the screen.
///
/// The terminal is put into raw mode for exactly as long as the answer is
/// being typed, so the characters themselves are not echoed. One asterisk is
/// drawn per character, because seeing the length is what catches a mis-paste
/// before a failed sign-in does. Raw mode is restored before
/// this returns by any path, including the one where the read fails: leaving
/// a person's terminal in raw mode is worse than failing to read a password.
///
/// As with [`ask`], with no terminal there is nobody to ask, and the failure
/// names the flag and the environment variable that would have answered.
pub fn ask_secret(question: &Question<'_>, theme: Theme) -> Result<String> {
    if !std::io::stdin().is_terminal() {
        bail!(unanswerable_message(question));
    }
    let mut output = std::io::stderr();
    write!(output, "{} ", theme.prompt(question.label)).context("writing the question")?;
    output.flush().context("writing the question")?;

    let typed = read_without_echo();
    eprintln!();
    let typed = typed?;
    if typed.trim().is_empty() {
        bail!("no {}: nothing was typed", question.subject);
    }
    Ok(typed)
}

/// Reads one line with the terminal's echo switched off.
fn read_without_echo() -> Result<String> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    enable_raw_mode().context("switching the terminal echo off")?;
    let outcome = collect_secret();
    // Restored before the result is examined, so a failure cannot leave the
    // terminal unusable.
    let restored = disable_raw_mode().context("switching the terminal echo back on");
    let typed = outcome?;
    restored?;
    Ok(typed)
}

/// What one keystroke does while a secret is being typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keystroke {
    /// Add this character.
    Add(char),
    /// Remove the last one.
    Remove,
    /// The answer is finished.
    Submit,
    /// Abandon the command.
    Cancel,
    /// Nothing this loop understands.
    Ignore,
}

/// What a keystroke means while typing a secret.
///
/// Pure, so the two that matter are tested rather than tried: Ctrl-C
/// abandons the command instead of submitting what has been typed so far, and
/// a control character that happens to carry a letter is not that letter.
#[must_use]
pub const fn interpret_keystroke(code: KeyCode, modifiers: KeyModifiers) -> Keystroke {
    if modifiers.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char('c' | 'd') => Keystroke::Cancel,
            // Ctrl-U clears the line, as it does in a shell.
            KeyCode::Char('u') => Keystroke::Remove,
            _ => Keystroke::Ignore,
        };
    }
    match code {
        KeyCode::Enter => Keystroke::Submit,
        KeyCode::Backspace | KeyCode::Delete => Keystroke::Remove,
        KeyCode::Esc => Keystroke::Cancel,
        KeyCode::Char(character) => Keystroke::Add(character),
        _ => Keystroke::Ignore,
    }
}

/// The mask drawn in place of each character of a secret.
pub const MASK: char = '*';

/// The key loop behind [`read_without_echo`], with the terminal already raw.
///
/// One asterisk per character goes to stderr as it is typed. Showing the
/// length is the difference between noticing a mis-paste and finding out from
/// a failed sign-in, and the length of a password is not the part worth
/// hiding from somebody looking over your shoulder.
fn collect_secret() -> Result<String> {
    use crossterm::event::read;

    let mut typed = String::new();
    loop {
        let Event::Key(KeyEvent {
            code,
            modifiers,
            kind,
            ..
        }) = read().context("reading the answer")?
        else {
            continue;
        };
        // Windows reports press and release; echoing both would double every
        // asterisk.
        if kind != KeyEventKind::Press {
            continue;
        }
        match interpret_keystroke(code, modifiers) {
            Keystroke::Submit => return Ok(typed),
            Keystroke::Cancel => bail!("cancelled"),
            Keystroke::Add(character) => {
                typed.push(character);
                echo(MASK);
            }
            Keystroke::Remove => {
                if typed.pop().is_some() {
                    // Back up, paint over the asterisk, back up again.
                    echo_raw("\u{8} \u{8}");
                }
            }
            Keystroke::Ignore => {}
        }
    }
}

/// Draws one character of the mask. A terminal that will not take it is not
/// worth failing the sign-in over, so the result is dropped.
fn echo(character: char) {
    echo_raw(&character.to_string());
}

fn echo_raw(text: &str) {
    let mut output = std::io::stderr();
    let _ = output.write_all(text.as_bytes());
    let _ = output.flush();
}

/// Asks a yes-or-no question. With no terminal, the default stands: a command
/// that cannot ask must not block, and the caller chose the default knowing
/// that.
pub fn confirm(label: &str, default: bool, theme: Theme) -> Result<bool> {
    if !std::io::stdin().is_terminal() {
        return Ok(default);
    }
    let hint = if default { "Y/n" } else { "y/N" };
    let mut output = std::io::stderr();
    write!(output, "{} {} ", theme.prompt(label), theme.muted(hint))
        .context("writing the question")?;
    output.flush().context("writing the question")?;

    let mut line = String::new();
    let read = std::io::stdin()
        .read_line(&mut line)
        .context("reading the answer")?;
    if read == 0 {
        return Ok(default);
    }
    Ok(interpret_confirmation(&line, default))
}

/// What a typed yes-or-no answer means. Anything that is not a yes or a no
/// takes the default rather than asking again: a confirmation is not worth a
/// second round trip.
#[must_use]
pub fn interpret_confirmation(input: &str, default: bool) -> bool {
    match input.trim().to_lowercase().as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        _ => default,
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
    fn a_typed_character_becomes_one_asterisk_of_the_mask() {
        assert_eq!(
            interpret_keystroke(KeyCode::Char('h'), KeyModifiers::NONE),
            Keystroke::Add('h')
        );
        assert_eq!(
            interpret_keystroke(KeyCode::Char('H'), KeyModifiers::SHIFT),
            Keystroke::Add('H'),
            "shift is how a capital arrives, not a control key"
        );
    }

    #[test]
    fn control_c_abandons_rather_than_submitting_what_was_typed_so_far() {
        assert_eq!(
            interpret_keystroke(KeyCode::Char('c'), KeyModifiers::CONTROL),
            Keystroke::Cancel
        );
        assert_eq!(
            interpret_keystroke(KeyCode::Char('d'), KeyModifiers::CONTROL),
            Keystroke::Cancel
        );
        assert_eq!(
            interpret_keystroke(KeyCode::Esc, KeyModifiers::NONE),
            Keystroke::Cancel
        );
    }

    #[test]
    fn a_control_character_carrying_a_letter_is_not_that_letter() {
        // The trap this guards: `Ctrl-a` must not silently become an `a` in
        // somebody's password.
        assert_eq!(
            interpret_keystroke(KeyCode::Char('a'), KeyModifiers::CONTROL),
            Keystroke::Ignore
        );
    }

    #[test]
    fn backspace_takes_a_character_back() {
        assert_eq!(
            interpret_keystroke(KeyCode::Backspace, KeyModifiers::NONE),
            Keystroke::Remove
        );
        assert_eq!(
            interpret_keystroke(KeyCode::Char('u'), KeyModifiers::CONTROL),
            Keystroke::Remove
        );
    }

    #[test]
    fn enter_finishes_the_answer() {
        assert_eq!(
            interpret_keystroke(KeyCode::Enter, KeyModifiers::NONE),
            Keystroke::Submit
        );
    }

    #[test]
    fn a_key_with_no_meaning_here_is_ignored() {
        for code in [KeyCode::Up, KeyCode::F(1), KeyCode::Tab] {
            assert_eq!(
                interpret_keystroke(code, KeyModifiers::NONE),
                Keystroke::Ignore,
                "{code:?}"
            );
        }
    }

    #[test]
    fn a_confirmation_takes_a_yes_or_a_no_and_defaults_otherwise() {
        assert!(interpret_confirmation("y\n", false));
        assert!(interpret_confirmation("YES", false));
        assert!(!interpret_confirmation("n", true));
        assert!(!interpret_confirmation("No\n", true));
        assert!(
            interpret_confirmation("\n", true),
            "an empty answer takes the default"
        );
        assert!(
            !interpret_confirmation("maybe", false),
            "so does anything else"
        );
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

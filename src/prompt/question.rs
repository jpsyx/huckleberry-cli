//! Questions and their pure interpretation.
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
    /// Guidance shown beneath the question in the muted hint colour.
    pub help: Option<&'a str>,
    /// The flag or argument that answers this without being asked.
    pub flag: &'a str,
    /// The answers on offer. Empty means free text.
    pub choices: &'a [Choice<'a>],
    /// What an empty answer means, if anything.
    pub default: Option<&'a str>,
    /// Whether an empty answer is itself an answer: leave the value out.
    pub skippable: bool,
}

impl<'a> Question<'a> {
    /// A free-text question with no default.
    #[must_use]
    pub const fn new(subject: &'a str, label: &'a str, flag: &'a str) -> Self {
        Self {
            subject,
            label,
            help: None,
            flag,
            choices: &[],
            default: None,
            skippable: false,
        }
    }

    /// Adds secondary guidance beneath the question.
    #[must_use]
    pub const fn with_help(mut self, help: &'a str) -> Self {
        self.help = Some(help);
        self
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

    /// Marks the value as one the record can do without, so that pressing
    /// Enter leaves it out rather than asking again.
    #[must_use]
    pub const fn optional(mut self) -> Self {
        self.skippable = true;
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
    let hint = if question.skippable && question.default.is_none() {
        format!(" {}", theme.muted("(enter to skip)"))
    } else {
        String::new()
    };
    let help = question
        .help
        .map_or_else(String::new, |text| format!("{}\n", theme.muted(text)));
    format!(
        "{}{hint}\n{help}{}{answer_line}",
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

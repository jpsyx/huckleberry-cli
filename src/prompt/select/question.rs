//! Resolves defaults and optional answers without terminal I/O.
use super::MenuItem;
use crate::prompt::Question;

/// A menu answer, separate from its display text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// The exact value the caller receives.
    Value(String),
    /// Leave this field empty.
    Skip,
    /// Open a text editor.
    Text,
}

/// Prepared choices for a prompt.
pub struct QuestionMenu {
    /// Display rows.
    pub items: Vec<MenuItem>,
    /// Parallel answers, preserving Skip distinctly.
    pub answers: Vec<Answer>,
    /// Highlighted default.
    pub default: usize,
}
impl QuestionMenu {
    fn push(&mut self, label: String, answer: Answer) {
        self.items.push(MenuItem {
            label,
            detail: None,
        });
        self.answers.push(answer);
    }
}

/// Builds the finite choices, including optional and unlisted defaults.
#[must_use]
pub fn question_menu(question: &Question<'_>) -> QuestionMenu {
    let mut menu = QuestionMenu {
        items: Vec::new(),
        answers: Vec::new(),
        default: 0,
    };
    if question.skippable && question.default.is_none() {
        menu.push("Skip".into(), Answer::Skip);
    }
    if let Some(default) = question.default
        && !question
            .choices
            .iter()
            .any(|choice| choice.value == default)
    {
        menu.push(
            format!("Keep current value ({default})"),
            Answer::Value(default.into()),
        );
    }
    for choice in question.choices {
        let label = if choice.value == "-" {
            "Clear value".into()
        } else if choice.hint.is_empty() {
            choice.value.into()
        } else {
            format!("{} ({})", choice.hint, choice.value)
        };
        menu.push(label, Answer::Value(choice.value.into()));
    }
    if question.choices.is_empty() {
        menu.push("Enter text".into(), Answer::Text);
    }
    if question.skippable
        && question.default.is_some()
        && !question.choices.iter().any(|choice| choice.value == "-")
    {
        menu.push("Clear value".into(), Answer::Skip);
    }
    if let Some(default) = question.default {
        menu.default = menu
            .answers
            .iter()
            .position(|answer| *answer == Answer::Value(default.into()))
            .unwrap_or(0);
    }
    menu
}

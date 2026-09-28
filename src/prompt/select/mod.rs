//! Numbered selection controls shared by prompts and command menus.
mod draw;
mod model;
pub use draw::render;
pub use model::{MenuItem, Selection, SelectionAction};
mod question;
mod terminal;
pub use question::{Answer, QuestionMenu, question_menu};
pub use terminal::choose;

/// Asks a prepared question and interprets its selection.
pub fn answer(
    question: &crate::prompt::Question<'_>,
    theme: crate::theme::Theme,
) -> anyhow::Result<Option<String>> {
    let menu = question_menu(question);
    let index = choose(question.label, &menu.items, menu.default, theme)?;
    match &menu.answers[index] {
        Answer::Value(value) => Ok(Some(value.clone())),
        Answer::Skip => Ok(None),
        Answer::Text => {
            let value = super::text::read(question, theme, false)?;
            Ok((!value.trim().is_empty()).then(|| value.trim().to_owned()))
        }
    }
}
pub(super) use draw::clipped as clip;

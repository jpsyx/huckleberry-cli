//! Owned controls reuse the same models and renderers as standalone prompts.
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::FormValue;
use crate::prompt::{
    Question,
    host::Input,
    select::{self, MenuItem, Selection, SelectionAction},
    text,
};
use crate::theme::Theme;

/// Navigation is distinct from accepting the value currently being edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlEvent {
    /// Redraw the same view.
    Stay,
    /// Validate and accept the current value.
    Accept,
    /// Return to the previous view, retaining the unfinished value.
    Back,
    /// Abandon the draft and go Home.
    Home,
    /// Cancel the draft at its launching menu.
    Cancel,
}

/// A view with its retained, editable control state.
pub struct View {
    label: String,
    control: Control,
}

enum Control {
    Choice {
        rows: Vec<MenuItem>,
        values: Vec<FormValue>,
        selection: Selection,
    },
    Text {
        typed: String,
        secret: bool,
    },
}

impl View {
    /// Creates a finite-choice view, retaining values independently of labels.
    #[must_use]
    pub fn create_choices(
        label: impl Into<String>,
        choices: Vec<(String, FormValue)>,
        default: usize,
    ) -> Self {
        let selection = Selection::new(choices.len(), default);
        let (rows, values) = choices
            .into_iter()
            .map(|(label, value)| {
                (
                    MenuItem {
                        label,
                        detail: None,
                    },
                    value,
                )
            })
            .unzip();
        Self {
            label: label.into(),
            control: Control::Choice {
                rows,
                values,
                selection,
            },
        }
    }

    /// Creates an editable text field with the previous value in its buffer.
    #[must_use]
    pub fn create_text(label: impl Into<String>, value: &str, secret: bool) -> Self {
        Self {
            label: label.into(),
            control: Control::Text {
                typed: value.into(),
                secret,
            },
        }
    }

    /// Applies input using the shared control implementations.
    pub fn apply(&mut self, input: Input, _now: f64, size: (u16, u16)) -> ControlEvent {
        match input {
            Input::Pasted(pasted) => {
                if let Control::Text { typed, .. } = &mut self.control {
                    text::paste(typed, &pasted);
                }
                ControlEvent::Stay
            }
            Input::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    return ControlEvent::Stay;
                }
                if key.code == KeyCode::Esc {
                    return ControlEvent::Home;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('c' | 'd'))
                {
                    return ControlEvent::Cancel;
                }
                if key.code == KeyCode::Left
                    || (matches!(self.control, Control::Choice { .. })
                        && matches!(key.code, KeyCode::Char('h' | 'H' | 'a' | 'A')))
                {
                    return ControlEvent::Back;
                }
                self.apply_control(key, size)
            }
        }
    }

    fn apply_control(&mut self, key: crossterm::event::KeyEvent, size: (u16, u16)) -> ControlEvent {
        match &mut self.control {
            Control::Choice {
                rows, selection, ..
            } => match selection.apply(key, rows.len(), usize::from(size.1).saturating_sub(2)) {
                SelectionAction::Submit(_) => ControlEvent::Accept,
                SelectionAction::Cancel => ControlEvent::Cancel,
                SelectionAction::Stay => ControlEvent::Stay,
            },
            Control::Text { typed, .. } => match text::typing(key, typed) {
                text::Typing::Done => ControlEvent::Accept,
                text::Typing::Stay => ControlEvent::Stay,
                text::Typing::Cancel => ControlEvent::Cancel,
            },
        }
    }

    /// Reads the control's value without altering it.
    pub fn value(&self, _now: f64) -> Result<FormValue> {
        match &self.control {
            Control::Choice {
                values, selection, ..
            } => values
                .get(selection.cursor)
                .cloned()
                .ok_or_else(|| anyhow!("there are no choices available")),
            Control::Text { typed, .. } => Ok(FormValue::Text(typed.clone())),
        }
    }

    /// Draws through the same functions used by the corresponding prompt.
    #[must_use]
    pub fn render(&self, _now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        match &self.control {
            Control::Choice {
                rows, selection, ..
            } => select::render(&self.label, rows, selection, size.0, size.1, theme),
            Control::Text { typed, secret } => text::lines(
                &Question::new("value", &self.label, ""),
                typed,
                *secret,
                theme,
            ),
        }
    }
}

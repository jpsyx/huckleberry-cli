//! Owned controls reuse the same models and renderers as standalone prompts.
use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::{FormValue, measurement::MeasurementState, time::TimeState, volume::VolumeState};
use crate::prompt::{
    Question,
    host::Input,
    select::{self, MenuItem, Selection, SelectionAction},
    text,
};
use crate::theme::{Theme, Tone};

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
    description: Option<(String, Tone)>,
    control: Control,
}

enum Control {
    Listing(super::listing::ListingControl),
    Choice {
        rows: Vec<MenuItem>,
        values: Vec<FormValue>,
        selection: Selection,
    },
    Text {
        typed: String,
        secret: bool,
    },
    Measurement(MeasurementState),
    Time(TimeState),
    Volume(VolumeState),
}

impl View {
    /// Creates a retained searchable list using the shared listing renderer.
    #[must_use]
    pub const fn create_listing(listing: super::listing::ListingControl) -> Self {
        Self {
            label: String::new(),
            description: None,
            control: Control::Listing(listing),
        }
    }

    /// Creates a number input whose buffer retains its physical units.
    #[must_use]
    pub fn create_measurement(label: impl Into<String>, measurement: MeasurementState) -> Self {
        Self {
            label: label.into(),
            description: None,
            control: Control::Measurement(measurement),
        }
    }

    /// Creates a time view retaining the exact accepted instant.
    #[must_use]
    pub fn create_time(label: impl Into<String>, time: TimeState) -> Self {
        Self {
            label: label.into(),
            description: None,
            control: Control::Time(time),
        }
    }

    /// Creates a volume view retaining quantity across unit changes.
    #[must_use]
    pub fn create_volume(label: impl Into<String>, volume: VolumeState) -> Self {
        Self {
            label: label.into(),
            description: None,
            control: Control::Volume(volume),
        }
    }
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
            description: None,
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
            description: None,
            control: Control::Text {
                typed: value.into(),
                secret,
            },
        }
    }

    /// Keeps contextual explanations with their view across suspension.
    #[must_use]
    pub fn with_description(mut self, description: String, tone: Tone) -> Self {
        self.description = Some((description, tone));
        self
    }

    /// Refreshes model-owned constraints without replacing an unfinished answer.
    pub(super) fn refresh(&mut self, fresh: Self) {
        self.label = fresh.label;
        self.description = fresh.description;
        match (&mut self.control, fresh.control) {
            (Control::Listing(listing), Control::Listing(fresh)) => listing.refresh(fresh),
            (Control::Time(time), Control::Time(fresh_time)) => {
                time.refresh_constraints(&fresh_time);
            }
            (Control::Measurement(value), Control::Measurement(fresh_value)) => {
                value.refresh(&fresh_value);
            }
            (
                Control::Choice {
                    rows,
                    values,
                    selection,
                },
                Control::Choice {
                    rows: fresh_rows,
                    values: fresh_values,
                    selection: fresh_selection,
                },
            ) => {
                selection.cursor = values
                    .get(selection.cursor)
                    .and_then(|value| fresh_values.iter().position(|fresh| fresh == value))
                    .unwrap_or(fresh_selection.cursor);
                selection.top = selection.top.min(selection.cursor);
                *rows = fresh_rows;
                *values = fresh_values;
            }
            _ => {}
        }
    }

    /// Applies input using the shared control implementations.
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> ControlEvent {
        if let Control::Listing(listing) = &mut self.control {
            return listing.apply(input, size);
        }
        match input {
            Input::Pasted(pasted) => {
                match &mut self.control {
                    Control::Text { typed, .. } => text::paste(typed, &pasted),
                    Control::Measurement(value) => text::paste(&mut value.typed, &pasted),
                    _ => {}
                }
                ControlEvent::Stay
            }
            Input::Key(key) => {
                match &mut self.control {
                    Control::Time(time) => return time.apply(key, now),
                    Control::Volume(volume) => return volume.apply(key),
                    _ => {}
                }
                if let Some(event) =
                    navigation(key, true, matches!(self.control, Control::Choice { .. }))
                {
                    return event;
                }
                self.apply_control(key, size)
            }
        }
    }

    fn apply_control(&mut self, key: crossterm::event::KeyEvent, size: (u16, u16)) -> ControlEvent {
        match &mut self.control {
            Control::Choice {
                rows, selection, ..
            } => match selection.apply(
                forward_key(key),
                rows.len(),
                usize::from(size.1).saturating_sub(2),
            ) {
                SelectionAction::Submit(_) => ControlEvent::Accept,
                SelectionAction::Cancel => ControlEvent::Cancel,
                SelectionAction::Stay => ControlEvent::Stay,
            },
            Control::Text { typed, .. } | Control::Measurement(MeasurementState { typed, .. }) => {
                match text::typing(key, typed) {
                    text::Typing::Done => ControlEvent::Accept,
                    text::Typing::Stay => ControlEvent::Stay,
                    text::Typing::Cancel => ControlEvent::Cancel,
                }
            }
            Control::Time(_) | Control::Volume(_) | Control::Listing(_) => ControlEvent::Stay,
        }
    }

    /// Reads the control's value without altering it.
    pub fn value(&self, now: f64) -> Result<FormValue> {
        match &self.control {
            Control::Listing(listing) => listing.value(),
            Control::Choice {
                values, selection, ..
            } => values
                .get(selection.cursor)
                .cloned()
                .ok_or_else(|| anyhow!("there are no choices available")),
            Control::Text { typed, .. } => Ok(FormValue::Text(typed.clone())),
            Control::Measurement(value) => value.value(),
            Control::Time(time) => time.value(now).map(FormValue::Instant),
            Control::Volume(volume) => {
                let (units, amount) = volume.value();
                Ok(FormValue::Volume { units, amount })
            }
        }
    }

    /// Freezes time defaults only after the form has accepted the answer.
    pub(super) fn accepted(&mut self, now: f64) {
        if let Control::Time(time) = &mut self.control {
            let _ = time.accept(now);
        }
    }

    /// Draws through the same functions used by the corresponding prompt.
    #[must_use]
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        if let Control::Listing(listing) = &self.control {
            return listing.render(size, theme);
        }
        let mut lines = Vec::new();
        if let Some((description, tone)) = &self.description {
            lines.extend(
                select::wrapped(description, usize::from(size.0), 0)
                    .into_iter()
                    .map(|line| theme.paint(*tone, &line)),
            );
        }
        let height = size
            .1
            .saturating_sub(u16::try_from(lines.len()).unwrap_or(u16::MAX))
            .saturating_sub(1);
        lines.extend(match &self.control {
            Control::Listing(listing) => listing.render(size, theme),
            Control::Choice {
                rows, selection, ..
            } => select::render(&self.label, rows, selection, size.0, height, theme),
            Control::Text { typed, secret } => text::lines(
                &Question::new("value", &self.label, ""),
                typed,
                *secret,
                theme,
            ),
            Control::Measurement(value) => value.render(&self.label, theme),
            Control::Time(time) => time.render(&self.label, now, size.0, theme),
            Control::Volume(volume) => volume.render(&self.label, size.0, theme),
        });
        let back = if matches!(self.control, Control::Text { .. } | Control::Measurement(_)) {
            "← back"
        } else {
            "← h/a back"
        };
        lines.push(theme.muted(&format!("{back} · Enter set · Esc home")));
        lines
    }
}

/// Navigation before the control interprets ordinary input.
pub(super) fn navigation(
    key: crossterm::event::KeyEvent,
    back: bool,
    letters: bool,
) -> Option<ControlEvent> {
    if key.kind != KeyEventKind::Press {
        return Some(ControlEvent::Stay);
    }
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return matches!(key.code, KeyCode::Char('c' | 'd')).then_some(ControlEvent::Cancel);
    }
    match key.code {
        KeyCode::Esc => Some(ControlEvent::Home),
        KeyCode::Left if back => Some(ControlEvent::Back),
        KeyCode::Char('h' | 'H' | 'a' | 'A') if back && letters => Some(ControlEvent::Back),
        _ => None,
    }
}

const fn forward_key(mut key: crossterm::event::KeyEvent) -> crossterm::event::KeyEvent {
    if !key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(
            key.code,
            KeyCode::Right | KeyCode::Char('l' | 'L' | 'd' | 'D' | ' ')
        )
    {
        key.code = KeyCode::Enter;
    }
    key
}

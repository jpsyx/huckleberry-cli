//! A view cursor changes without discarding earlier or later answers.
use std::collections::BTreeMap;

use anyhow::{Result, anyhow};

use super::{ControlEvent, View};
use crate::{cli::Units, prompt::host::Input, theme::Theme};

/// Stable field identity, optionally scoped to an entry or repeated item.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ViewId {
    /// The purpose of the field, independent of its displayed label.
    pub field: &'static str,
    /// The owner of a repeated field, such as a history entry token.
    pub owner: Option<String>,
}

/// An accepted control value, before a form interprets its meaning.
#[derive(Clone, PartialEq)]
pub enum FormValue {
    /// The semantic value of a selected row.
    Choice(String),
    /// A text buffer, which may be empty.
    Text(String),
    /// An explicit omitted or cleared value.
    Skipped,
    /// A resolved timestamp, including its original precision.
    Instant(f64),
    /// A quantity and the units in which it was accepted.
    Volume {
        /// Display units.
        units: Units,
        /// The accepted amount in those units.
        amount: f64,
    },
    /// A selected item's stable identity.
    Picked(String),
}

/// What the shell does after one form input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormEvent {
    /// Continue editing the form.
    Stay,
    /// Retain the draft at its launching menu.
    Suspend,
    /// Discard the draft and return to Home.
    Home,
    /// Discard the draft and return to the launching menu.
    Cancel,
    /// A validated submission is ready for execution.
    Ready,
}

/// A command's answers and routing, independent of I/O and the clock.
pub trait FormModel {
    /// The typed input consumed by the command's executor.
    type Output;
    /// Applicable views in order, including containing menus.
    fn route(&self) -> Vec<ViewId>;
    /// Builds a view's initial controls from the draft and supplied clock.
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View>;
    /// Validates an answer before updating the draft.
    fn accept(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()>;
    /// The following view, or completion after the last applicable answer.
    fn forward(&self, id: &ViewId) -> Option<ViewId> {
        let route = self.route();
        route
            .iter()
            .position(|view| view == id)
            .and_then(|index| route.get(index + 1))
            .cloned()
    }
    /// Validates and constructs the submission from applicable answers only.
    fn complete(&self) -> Result<Self::Output>;
}

/// A form and its retained controls. Values are not exposed through Debug.
pub struct FormState<Form: FormModel> {
    model: Form,
    views: BTreeMap<ViewId, View>,
    current: ViewId,
    trouble: Option<String>,
}

impl<Form: FormModel> FormState<Form> {
    /// Opens the first applicable view of a form.
    pub fn create_form(model: Form, now: f64) -> Result<Self> {
        let current = model
            .route()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("form has no views"))?;
        let view = model.create_view(&current, now)?;
        Ok(Self {
            model,
            views: BTreeMap::from([(current.clone(), view)]),
            current,
            trouble: None,
        })
    }

    /// Takes one input without reading the clock or touching the terminal.
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent {
        self.trouble = None;
        let event = self
            .views
            .get_mut(&self.current)
            .expect("current view exists")
            .apply(input, now, size);
        let result = match event {
            ControlEvent::Stay => return FormEvent::Stay,
            ControlEvent::Home => return FormEvent::Home,
            ControlEvent::Cancel => return FormEvent::Cancel,
            ControlEvent::Back => self.back(now),
            ControlEvent::Accept => self.accept(now),
        };
        match result {
            Ok(event) => event,
            Err(error) => {
                self.trouble = Some(format!("{error:#}"));
                FormEvent::Stay
            }
        }
    }

    fn back(&mut self, now: f64) -> Result<FormEvent> {
        let route = self.model.route();
        let previous = route
            .iter()
            .position(|view| view == &self.current)
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| route.get(index));
        let Some(previous) = previous else {
            return Ok(FormEvent::Suspend);
        };
        self.open(previous.clone(), now)?;
        Ok(FormEvent::Stay)
    }

    fn accept(&mut self, now: f64) -> Result<FormEvent> {
        let value = self.views[&self.current].value(now)?;
        self.model.accept(&self.current, value, now)?;
        self.views
            .get_mut(&self.current)
            .expect("current view exists")
            .accepted(now);
        if let Some(following) = self.model.forward(&self.current) {
            self.open(following, now)?;
            Ok(FormEvent::Stay)
        } else {
            self.model.complete()?;
            Ok(FormEvent::Ready)
        }
    }

    fn open(&mut self, id: ViewId, now: f64) -> Result<()> {
        if !self.views.contains_key(&id) {
            self.views
                .insert(id.clone(), self.model.create_view(&id, now)?);
        }
        self.current = id;
        Ok(())
    }

    /// Renders the current control and any validation error.
    #[must_use]
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        let mut lines = self.views[&self.current].render(now, size, theme);
        if let Some(trouble) = &self.trouble {
            lines.push(theme.warning(trouble));
        }
        lines
    }

    /// Returns validated command inputs; creating them does not execute them.
    pub fn complete(&self) -> Result<Form::Output> {
        self.model.complete()
    }
}

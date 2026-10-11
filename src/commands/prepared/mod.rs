//! Owned collection and execution boundaries for shell commands.
mod execute;
mod forms;
mod gates;
mod utilities;
pub use gates::{prepare, startup};

use crate::{
    prompt::{
        flow::{FormEvent, FormModel, FormState},
        host::Input,
    },
    theme::Theme,
};
use anyhow::Result;
use std::{future::Future, pin::Pin};

/// A consumed command's work, with no prompt collection inside it.
pub type Execution = Pin<Box<dyn Future<Output = Result<CommandProgress>> + Send>>;

/// Either a finished action or the next form after a committed prerequisite.
pub enum CommandProgress {
    /// The action finished; recording actions return Home.
    Finished {
        /// Whether to return Home after reading its receipt.
        home: bool,
    },
    /// A prerequisite completed, and this owned form continues the action.
    Continue(Box<PreparedCommand>),
}

/// Owns a typed form and its consuming executor without retaining prompt workers.
pub struct PreparedCommand {
    form: Box<dyn forms::PreparedForm>,
}

impl PreparedCommand {
    /// Binds typed answers to the only operation allowed to submit them.
    pub fn create_prepared<Form, Execute, Work>(
        model: Form,
        now: f64,
        execute: Execute,
    ) -> Result<Self>
    where
        Form: FormModel + Send + 'static,
        Form::Output: Send + 'static,
        Execute: FnOnce(Form::Output) -> Work + Send + 'static,
        Work: Future<Output = Result<CommandProgress>> + Send + 'static,
    {
        let state = if model.route().is_empty() {
            forms::Collection::Answer(model.complete()?)
        } else {
            forms::Collection::Form(FormState::create_form(model, now)?)
        };
        Ok(Self {
            form: Box::new(forms::Prepared { state, execute }),
        })
    }

    /// Whether all inputs were supplied and no collection is necessary.
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.form.is_ready()
    }

    /// Applies one key to the owned form, without performing I/O.
    pub fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent {
        self.form.apply(input, now, size)
    }

    /// Draws the current view from retained values.
    #[must_use]
    pub fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        self.form.render(now, size, theme)
    }

    /// Consumes the form so navigation cannot replay a submission.
    #[must_use]
    pub fn execute(self) -> Execution {
        self.form.execute()
    }
}

#[cfg(test)]
mod tests;

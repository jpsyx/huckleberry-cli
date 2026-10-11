//! Type erasure keeps the shell independent of each command's typed submission.
use super::{CommandProgress, Execution};
use crate::{
    prompt::{
        flow::{FormEvent, FormModel, FormState},
        host::Input,
    },
    theme::Theme,
};
use anyhow::Result;
use std::future::Future;

pub(super) trait PreparedForm: Send {
    fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent;
    fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String>;
    fn execute(self: Box<Self>) -> Execution;
}

pub(super) struct Prepared<Form: FormModel, Execute> {
    pub state: FormState<Form>,
    pub execute: Execute,
}

impl<Form, Execute, Work> PreparedForm for Prepared<Form, Execute>
where
    Form: FormModel + Send + 'static,
    Execute: FnOnce(Form::Output) -> Work + Send + 'static,
    Work: Future<Output = Result<CommandProgress>> + Send + 'static,
{
    fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent {
        self.state.apply(input, now, size)
    }
    fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        self.state.render(now, size, theme)
    }
    fn execute(self: Box<Self>) -> Execution {
        match self.state.complete() {
            Ok(submission) => Box::pin((self.execute)(submission)),
            Err(error) => Box::pin(std::future::ready(Err(error))),
        }
    }
}

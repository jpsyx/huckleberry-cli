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
    fn is_ready(&self) -> bool;
}

pub(super) struct Prepared<Form: FormModel, Execute> {
    pub state: Collection<Form>,
    pub execute: Execute,
}

pub(super) enum Collection<Form: FormModel> {
    Form(FormState<Form>),
    Answer(Form::Output),
}

impl<Form, Execute, Work> PreparedForm for Prepared<Form, Execute>
where
    Form: FormModel + Send + 'static,
    Form::Output: Send + 'static,
    Execute: FnOnce(Form::Output) -> Work + Send + 'static,
    Work: Future<Output = Result<CommandProgress>> + Send + 'static,
{
    fn apply(&mut self, input: Input, now: f64, size: (u16, u16)) -> FormEvent {
        match &mut self.state {
            Collection::Form(state) => state.apply(input, now, size),
            Collection::Answer(_) => FormEvent::Ready,
        }
    }
    fn render(&self, now: f64, size: (u16, u16), theme: Theme) -> Vec<String> {
        match &self.state {
            Collection::Form(state) => state.render(now, size, theme),
            Collection::Answer(_) => Vec::new(),
        }
    }
    fn is_ready(&self) -> bool {
        matches!(self.state, Collection::Answer(_))
    }
    fn execute(self: Box<Self>) -> Execution {
        let answer = match self.state {
            Collection::Form(state) => state.complete(),
            Collection::Answer(answer) => Ok(answer),
        };
        match answer {
            Ok(submission) => Box::pin((self.execute)(submission)),
            Err(error) => Box::pin(std::future::ready(Err(error))),
        }
    }
}

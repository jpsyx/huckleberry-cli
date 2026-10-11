//! Terminal transport for the same retained controls the shell drives directly.
use super::{FormEvent, FormModel, FormState};
use crate::{
    prompt::{
        self,
        host::{self, Input},
        terminal::TerminalGuard,
    },
    theme::Theme,
};
use anyhow::{Result, bail};
use crossterm::{
    event::{self, Event},
    terminal,
};
use huckleberry_api::client::now_seconds;

/// Collects a form through a host or the existing standalone terminal transport.
pub fn collect<Form: FormModel>(form: &mut FormState<Form>, theme: Theme) -> Result<Form::Output> {
    if host::hosted() {
        return collect_hosted(form, theme);
    }
    if !prompt::available() {
        bail!("interactive form needs terminal input and stderr");
    }
    let mut guard = TerminalGuard::enter()?;
    let result = collect_terminal(form, theme);
    guard.restore()?;
    result
}

fn collect_hosted<Form: FormModel>(
    form: &mut FormState<Form>,
    theme: Theme,
) -> Result<Form::Output> {
    loop {
        let size = host::size();
        let input = host::frame(form.render(now_seconds(), size, theme))?;
        match form.apply(input, now_seconds(), size) {
            FormEvent::Ready => return form.complete(),
            FormEvent::Stay => {}
            _ => return Err(prompt::Cancelled.into()),
        }
    }
}

fn collect_terminal<Form: FormModel>(
    form: &mut FormState<Form>,
    theme: Theme,
) -> Result<Form::Output> {
    let mut drawn = 0;
    let mut output = std::io::stderr();
    loop {
        let size = terminal::size().unwrap_or((80, 24));
        let lines = form.render(now_seconds(), size, theme);
        prompt::select::redraw(&mut output, &lines, drawn)?;
        drawn = lines.len();
        let input = match event::read()? {
            Event::Key(key) => Input::Key(key),
            Event::Paste(text) => Input::Pasted(text),
            _ => continue,
        };
        match form.apply(input, now_seconds(), size) {
            FormEvent::Stay => {}
            FormEvent::Ready => return form.complete(),
            _ => return Err(prompt::Cancelled.into()),
        }
    }
}

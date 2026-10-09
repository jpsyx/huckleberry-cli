//! Terminal and hosted input loops for the summary visualization.

use std::io::IsTerminal;

use anyhow::{Context as _, Result};
use crossterm::event::{self, Event};
use ratatui::widgets::Paragraph;

use crate::{
    prompt::host,
    render::summary::view::{State, View},
    theme::Theme,
};

/// A hosted panel or an interactive terminal can browse; redirected output stays plain.
#[must_use]
pub fn interactive() -> bool {
    host::hosted() || (std::io::stdin().is_terminal() && std::io::stdout().is_terminal())
}

/// Open the same visualization in the shell's panel or its own terminal.
pub fn show(view: &View<'_>, theme: Theme) -> Result<()> {
    if host::hosted() {
        return hosted(view, theme);
    }
    let mut terminal = ratatui::init();
    let outcome = browse(view, theme, &mut terminal);
    ratatui::restore();
    outcome
}

fn hosted(view: &View<'_>, theme: Theme) -> Result<()> {
    let mut state = State::default();
    loop {
        let lines = view.lines(&mut state, host::size(), theme);
        if let host::Input::Key(key) = host::frame(lines)?
            && state.apply(key)
        {
            // A browsable view has already offered its exit; return straight to the menu.
            return Err(crate::prompt::Cancelled.into());
        }
    }
}

fn browse(view: &View<'_>, theme: Theme, terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut state = State::default();
    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                let lines = view.lines(&mut state, (area.width, area.height), theme);
                let lines: Vec<_> = lines
                    .iter()
                    .map(|line| crate::tui::draw::painted(line))
                    .collect();
                frame.render_widget(Paragraph::new(lines), area);
            })
            .context("drawing the summary")?;
        if let Event::Key(key) = event::read().context("reading summary navigation")?
            && state.apply(key)
        {
            return Ok(());
        }
    }
}

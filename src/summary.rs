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
        let size = host::size();
        let mut lines = view.lines(&mut state, size, theme);
        if !state.is_comparing
            && let Some(hint) = lines.last_mut()
        {
            *hint = theme.muted(&crate::prompt::select::clip(
                "←→ h/l a/d · ↑↓ j/k w/s cell · Enter graph · Esc home",
                usize::from(size.0),
            ));
        }
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
                let lines = view.table_lines(&mut state, (area.width, area.height), theme);
                let lines: Vec<_> = lines
                    .iter()
                    .map(|line| crate::tui::draw::painted(line))
                    .collect();
                frame.render_widget(Paragraph::new(lines), area);
                if state.is_comparing {
                    let area = crate::tui::draw::summary::comparison_area(area);
                    let lines = view.lines(
                        &mut state,
                        (area.width.saturating_sub(2), area.height.saturating_sub(2)),
                        theme,
                    );
                    crate::tui::draw::summary::draw_comparison(frame, &lines);
                }
            })
            .context("drawing the summary")?;
        if let Event::Key(key) = event::read().context("reading summary navigation")?
            && state.apply(key)
        {
            return Ok(());
        }
    }
}

/// Resolves a comparison date against the discrete family day, rejecting future days.
pub fn parse_day(
    value: &str,
    calendar: &crate::domain::Calendar,
    rule: crate::domain::today::DayRule,
    now: f64,
) -> Result<jiff::civil::Date> {
    let today = rule.day_of(calendar, now);
    let day = match value {
        "today" => today,
        "yesterday" => calendar.offset_day(today, -1),
        value => value
            .parse()
            .context("comparison day must be today, yesterday, or YYYY-MM-DD")?,
    };
    anyhow::ensure!(day <= today, "comparison day cannot be in the future");
    Ok(day)
}

//! Draws the permanent Summary panel beside the menu or running command.

use ratatui::{Frame, layout::Rect, widgets::Paragraph};

use super::{App, Panel, painted, panel_block};

/// Reuses Summary's table, chart and viewport with the shell's current reading.
pub fn draw(frame: &mut Frame, area: Rect, app: &mut App, now: f64) {
    let outline = panel_block("Summary", Panel::Summary, app.focus);
    let inner = outline.inner(area);
    frame.render_widget(outline, area);
    let lines = app.facts.reading().map_or_else(
        || vec!["nothing read yet · press r".to_owned()],
        |reading| app.summary.lines(reading, (inner.width, inner.height), now),
    );
    frame.render_widget(
        Paragraph::new(lines.iter().map(|line| painted(line)).collect::<Vec<_>>()),
        inner,
    );
}

/// Shared centered overlay bounds for the shell and standalone Summary.
#[must_use]
pub const fn comparison_area(area: Rect) -> Rect {
    let horizontal = if area.width >= 60 { 2 } else { 0 };
    let vertical = if area.height >= 18 { 1 } else { 0 };
    Rect::new(
        area.x + horizontal,
        area.y + vertical,
        area.width.saturating_sub(horizontal * 2),
        area.height.saturating_sub(vertical * 2),
    )
}

/// Clears and draws an opaque comparison modal without taking terminal ownership.
pub fn draw_comparison(frame: &mut Frame, lines: &[String]) {
    let area = comparison_area(frame.area());
    let outline = super::block("Cumulative comparison");
    let inner = outline.inner(area);
    frame.render_widget(ratatui::widgets::Clear, area);
    frame.render_widget(outline, area);
    frame.render_widget(
        Paragraph::new(lines.iter().map(|line| painted(line)).collect::<Vec<_>>()),
        inner,
    );
}

/// The comparison owns an overlay while retaining both underlying panel states.
pub fn draw_modal(frame: &mut Frame, app: &App, now: f64) {
    if !app.summary.state.is_comparing {
        return;
    }
    if let Some(reading) = app.facts.reading() {
        let area = comparison_area(frame.area());
        let lines = app.summary.comparison_lines(
            reading,
            (area.width.saturating_sub(2), area.height.saturating_sub(2)),
            now,
        );
        draw_comparison(frame, &lines);
    }
}

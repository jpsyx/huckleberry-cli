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

//! The Menu view: the shell's main panel.
//!
//! Every row is numbered, the one under the cursor carries a bar down its
//! left, and a row that opens another menu says so with a chevron. The numbers
//! are one column wide for the whole menu, so every label starts in the same
//! place whether its row is 9 or 10.

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::theme::Tone;
use crate::tui::state::{App, Row};

use super::{block, tone};

/// The cursor, drawn as a bar down the left of the highlighted row.
const CURSOR: &str = "▌ ";

/// A row that opens another menu says so.
const CHEVRON: char = '›';

/// Draws the rows, inside a block that names the question they answer.
pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let outline = block(app.title());
    let inner = outline.inner(area);
    frame.render_widget(outline, area);

    let rows = app.rows();
    let digits = rows.len().to_string().len();
    let items = rows
        .iter()
        .enumerate()
        .map(|(index, row)| item(index, row, inner.width, digits))
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_symbol(CURSOR).highlight_style(
        Style::default()
            .fg(tone(Tone::Selected))
            .add_modifier(Modifier::BOLD),
    );
    let mut state = ListState::default()
        .with_selected(Some(app.cursor()))
        .with_offset(app.top());
    frame.render_stateful_widget(list, inner, &mut state);
}

/// One row: its number, its label, and a chevron when it opens a menu.
fn item(index: usize, row: &Row, width: u16, digits: usize) -> ListItem<'static> {
    let number = format!("{:>digits$}. ", index + 1);
    // The cursor symbol is drawn by the list, in the same width on every row.
    let room = usize::from(width).saturating_sub(CURSOR.chars().count() + number.chars().count());
    // Two columns held back: the chevron, and a space between it and the edge.
    let label = crate::prompt::select::clip(&row.label, room.saturating_sub(2));
    let mut spans = vec![
        Span::styled(number, Style::default().fg(tone(Tone::Muted))),
        Span::styled(label.clone(), Style::default().fg(tone(Tone::Value))),
    ];
    if row.opens_menu {
        let used = Line::raw(&label).width();
        spans.push(Span::raw(" ".repeat(room.saturating_sub(used + 2))));
        spans.push(Span::styled(
            CHEVRON.to_string(),
            Style::default().fg(tone(Tone::Muted)),
        ));
    }
    ListItem::new(Line::from(spans))
}

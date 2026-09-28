//! Menu lines with terminal-cell bounded labels.
use super::{MenuItem, Selection};
use crate::theme::Theme;

/// Clips text to terminal cells, keeping multibyte characters intact.
#[must_use]
pub fn clipped(text: &str, width: usize) -> String {
    let mut shown = String::new();
    for character in text.chars().filter(|character| !character.is_control()) {
        let candidate = format!("{shown}{character}");
        if ratatui::text::Line::raw(&candidate).width() > width {
            break;
        }
        shown.push(character);
    }
    shown
}

/// Renders the visible portion of a menu.
#[must_use]
pub fn render(
    label: &str,
    items: &[MenuItem],
    state: &Selection,
    width: u16,
    height: u16,
    theme: Theme,
) -> Vec<String> {
    let width = usize::from(width).max(1);
    let count = usize::from(height).saturating_sub(2).max(1);
    let top = crate::listing::state::scrolled(state.top, state.cursor, count, items.len());
    let mut lines = vec![theme.prompt(&clipped(label, width))];
    for (index, item) in items.iter().enumerate().skip(top).take(count) {
        let marker = if index == state.cursor { ">" } else { " " };
        let detail = item
            .detail
            .as_ref()
            .map_or_else(String::new, |text| format!(": {text}"));
        let line = clipped(
            &format!("{marker} {}. {}{detail}", index + 1, item.label),
            width,
        );
        lines.push(if index == state.cursor {
            theme.value(&line)
        } else {
            line
        });
    }
    lines.push(theme.muted(&clipped("↑/↓ j/k h/p · Enter selects · Esc back", width)));
    lines
}

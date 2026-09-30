//! Menu lines with terminal-cell bounded labels.
use super::{MenuItem, Selection};
use crate::theme::{Theme, Tone};

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
    let width = usize::from(width).saturating_sub(1).max(1);
    let count = usize::from(height).saturating_sub(2).max(1);
    let rows = item_lines(items, state.cursor, width);
    let mut top = state
        .top
        .min(state.cursor)
        .min(items.len().saturating_sub(1));
    while top < state.cursor && rows[top..=state.cursor].iter().map(Vec::len).sum::<usize>() > count
    {
        top += 1;
    }
    let mut lines = vec![theme.prompt(&clipped(label, width))];
    for (index, row) in rows.iter().enumerate().skip(top) {
        for line in row {
            if lines.len() > count {
                break;
            }
            lines.push(if index == state.cursor {
                theme.paint(Tone::Selected, line)
            } else {
                line.clone()
            });
        }
        if lines.len() > count {
            break;
        }
    }
    lines.push(theme.muted(&clipped(
        "↑/↓ j/k move · 1-9 highlight · Enter selects · ← h back",
        width,
    )));
    lines
}

fn wrapped(text: &str, width: usize, indent: usize) -> Vec<String> {
    let continuation = " ".repeat(indent.min(width.saturating_sub(1)));
    let mut lines = Vec::new();
    let mut line = String::new();
    for character in text.chars().filter(|character| !character.is_control()) {
        if ratatui::text::Line::raw(format!("{line}{character}")).width() > width {
            lines.push(std::mem::replace(&mut line, continuation.clone()));
        }
        if ratatui::text::Line::raw(format!("{line}{character}")).width() <= width {
            line.push(character);
        }
    }
    lines.push(line);
    lines
}

fn item_lines(items: &[MenuItem], cursor: usize, width: usize) -> Vec<Vec<String>> {
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let marker = if index == cursor { ">" } else { " " };
            let prefix = format!("{marker} {}. ", index + 1);
            let detail = item
                .detail
                .as_ref()
                .map_or_else(String::new, |text| format!(": {text}"));
            wrapped(
                &format!("{prefix}{}{detail}", item.label),
                width,
                prefix.len(),
            )
        })
        .collect()
}

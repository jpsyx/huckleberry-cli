//! Column navigation and a responsive summary with a graph kept below the table.

use std::ops::Range;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::{
    cli::Units,
    domain::{Calendar, summaries::DaySummary, types::Dataset},
    theme::{Theme, Tone},
};

use super::{
    chart,
    columns::{
        COLUMNS, data_cells, group_row, heading_cells, join_cells, join_heading_cells, subgroup_row,
    },
};

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;

/// Selection and vertical scroll, independent of terminal I/O.
#[derive(Default)]
pub struct State {
    /// Index of the selected numeric column; the day column cannot be selected.
    pub selected: usize,
    scroll: usize,
    first_column: usize,
    limit: usize,
    page: usize,
}

impl State {
    /// Apply one key, returning true when the view should close.
    pub fn apply(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return false;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return matches!(key.code, KeyCode::Char('c' | 'q'));
        }
        match key.code {
            KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('q' | 'Q') => return true,
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h' | 'H' | 'a' | 'A') => {
                self.previous_column();
            }
            KeyCode::Tab if key.modifiers.contains(KeyModifiers::SHIFT) => self.previous_column(),
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l' | 'L' | 'd' | 'D') => {
                self.selected = (self.selected + 1) % COLUMNS.len();
            }
            KeyCode::Down | KeyCode::Char('j' | 'J' | 's' | 'S') => {
                self.scroll = self.scroll.saturating_add(1);
            }
            KeyCode::Up | KeyCode::Char('k' | 'K' | 'w' | 'W') => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::PageDown => self.scroll = self.scroll.saturating_add(self.page),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(self.page),
            KeyCode::Home | KeyCode::Char('g') => self.scroll = 0,
            KeyCode::End | KeyCode::Char('G') => self.scroll = self.limit,
            _ => {}
        }
        self.scroll = self.scroll.min(self.limit);
        false
    }

    const fn previous_column(&mut self) {
        self.selected = (self.selected + COLUMNS.len() - 1) % COLUMNS.len();
    }
}

/// The data shown by either interactive host, with an explicit reading time.
pub struct View<'a> {
    /// The shell already displays reference ranges and their note in the Now drawer.
    pub is_hosted: bool,
    /// Newest-first daily aggregates.
    pub rows: &'a [DaySummary],
    /// Source of the child's name and configured night hours.
    pub dataset: &'a Dataset,
    /// Family timezone.
    pub calendar: &'a Calendar,
    /// Display units shared by table and graph.
    pub units: Units,
    /// Time used to label the reading, never read inside a renderer.
    pub now: f64,
}

impl View<'_> {
    /// A frame that fits its host. The day stays visible while numeric columns move.
    #[must_use]
    pub fn lines(&self, state: &mut State, size: (u16, u16), theme: Theme) -> Vec<String> {
        let width = usize::from(size.0);
        let height = usize::from(size.1);
        if height < 6 {
            let mut lines = vec!["Enlarge the panel".to_owned(), "Esc/q back".to_owned()];
            lines.truncate(height);
            lines.resize(height, String::new());
            return lines.into_iter().map(|line| clip(&line, width)).collect();
        }
        let range = visible_columns(state.first_column, state.selected, self.units, width);
        state.first_column = range.start;
        let head_height = if height < 12 { 3 } else { 4 };
        let footer_height = if height < 12 { 2 } else { 3 };
        let content_height = size.1.saturating_sub(head_height + footer_height);
        let minimum_chart_height = (size.1 / 3)
            .clamp(4, 9)
            .min(content_height.saturating_sub(1));
        let body = self.body(range.clone(), state.selected, width, theme);
        let body_height = usize::from(content_height - minimum_chart_height).min(body.len());
        let chart_height = content_height - body_height as u16;
        state.page = body_height.max(1);
        state.limit = body.len().saturating_sub(body_height);
        state.scroll = state.scroll.min(state.limit);
        let mut lines = self.head(range, state.selected, theme);
        if head_height == 3 {
            lines.remove(0);
        }
        lines.extend(body.into_iter().skip(state.scroll).take(body_height));
        if chart_height > 0 {
            lines.extend(chart::lines(
                self.rows,
                &COLUMNS[state.selected],
                self.units,
                (size.0, chart_height),
                theme,
            ));
        }
        lines.extend(self.status(state, width, footer_height == 2, theme));
        lines.truncate(height);
        lines.into_iter().map(|line| clip(&line, width)).collect()
    }

    fn status(&self, state: &State, width: usize, compact: bool, theme: Theme) -> Vec<String> {
        hints(state, width, compact)
            .into_iter()
            .chain([crate::render::now::as_of(self.dataset, self.now)])
            .map(|line| theme.muted(&line))
            .collect()
    }

    fn head(&self, range: Range<usize>, selected: usize, theme: Theme) -> Vec<String> {
        let mut cells = heading_cells(self.units, range.clone(), Some(selected));
        for (index, cell) in range.clone().zip(&mut cells) {
            *cell = theme.paint(
                if index == selected {
                    Tone::Prompt
                } else {
                    Tone::Heading
                },
                cell,
            );
        }
        let age = super::age_in_days(self.dataset, self.calendar, self.now)
            .map_or_else(String::new, |days| format!(" · {days} days old"));
        vec![
            theme.heading(&format!(
                "Summary · {}{age} · column {}/{} · ~ partial",
                self.dataset.child.name,
                selected + 1,
                COLUMNS.len()
            )),
            theme.heading(&group_row(self.units, range.clone())),
            theme.heading(&subgroup_row(self.units, range.clone())),
            join_heading_cells(&theme.heading("day        "), &cells, range),
        ]
    }

    fn body(
        &self,
        range: Range<usize>,
        selected: usize,
        width: usize,
        theme: Theme,
    ) -> Vec<String> {
        let mut lines: Vec<_> = self
            .rows
            .iter()
            .map(|row| {
                let mut cells = data_cells(row, self.units, range.clone());
                for (index, cell) in range.clone().zip(&mut cells) {
                    let tone = match (row.partial, index == selected) {
                        (true, true) => Tone::Prompt,
                        (true, false) => Tone::Today,
                        (false, true) => Tone::Accent,
                        (false, false) => Tone::Value,
                    };
                    *cell = theme.paint(tone, cell);
                }
                let day = super::format::pad(&super::day_label(row), 11);
                let day = theme.paint(
                    if row.partial {
                        Tone::Today
                    } else {
                        Tone::Value
                    },
                    &day,
                );
                join_cells(&day, &cells, range.clone())
            })
            .collect();
        for line in self.notes(theme) {
            lines.extend(wrap(&line, width));
        }
        lines
    }
    fn notes(&self, theme: Theme) -> Vec<String> {
        if self.is_hosted {
            super::notes(
                self.rows,
                self.dataset,
                theme,
                self.units,
                super::age_in_days(self.dataset, self.calendar, self.now),
            )
        } else {
            super::footer(
                self.rows,
                self.dataset,
                self.calendar,
                theme,
                self.units,
                self.now,
            )
        }
    }
}

fn hints(state: &State, width: usize, compact: bool) -> Vec<String> {
    if compact {
        return vec![if width < 55 {
            "Tab/←→ ↑↓ j/k · Esc/q back".into()
        } else {
            "Tab/←→ h/l a/d · ↑↓ j/k w/s · Esc/q back".into()
        }];
    }
    if width < 55 {
        return vec![
            "Tab/S-Tab ←/→ h/l a/d".into(),
            "↑↓ j/k w/s · Esc/q back".into(),
        ];
    }
    vec![
        "Tab/Shift-Tab · ←/→ h/l a/d columns · Esc/q back".into(),
        format!(
            "↑/↓ j/k w/s scroll rows & notes · {}/{}",
            state.scroll + 1,
            state.limit + 1
        ),
    ]
}

/// Keep the viewport anchored until selection passes an edge or resizing hides it.
fn visible_columns(first: usize, selected: usize, units: Units, width: usize) -> Range<usize> {
    let mut start = first.min(selected);
    while start < selected && table_width(start..selected + 1, units) > width {
        start += 1;
    }
    let mut end = selected + 1;
    while end < COLUMNS.len() && table_width(start..end + 1, units) <= width {
        end += 1;
    }
    start..end
}

fn table_width(range: Range<usize>, units: Units) -> usize {
    11 + range
        .map(|index| {
            COLUMNS[index].width(units)
                + if index == 0 || COLUMNS[index - 1].group != COLUMNS[index].group {
                    3
                } else {
                    2
                }
        })
        .sum::<usize>()
}

fn clip(line: &str, width: usize) -> String {
    let mut painted = crate::tui::draw::painted(line);
    let mut remaining = width;
    painted.spans.iter_mut().for_each(|span| {
        let mut text = String::new();
        for character in span.content.chars() {
            let cells = ratatui::text::Span::raw(character.to_string()).width();
            if cells > remaining {
                break;
            }
            remaining -= cells;
            text.push(character);
        }
        span.content = text.into();
    });
    painted.spans.iter().map(|span| paint_span(span)).collect()
}

fn paint_span(span: &ratatui::text::Span<'_>) -> String {
    let role = Tone::ALL
        .into_iter()
        .find(|tone| crate::tui::draw::style(*tone) == span.style);
    role.map_or_else(
        || span.content.to_string(),
        |role| Theme::dark(true).paint(role, &span.content),
    )
}

fn wrap(line: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let painted = crate::tui::draw::painted(line);
    let mut lines = vec![String::new()];
    let mut used = 0;
    for span in painted.spans {
        for character in span.content.chars() {
            let piece = ratatui::text::Span::styled(character.to_string(), span.style);
            let cells = piece.width();
            if used + cells > width {
                lines.push(String::new());
                used = 0;
            }
            lines
                .last_mut()
                .expect("one line exists")
                .push_str(&paint_span(&piece));
            used += cells;
        }
    }
    lines
}

//! The permanent Summary panel, using the standalone view and the shell's reading.

use crate::{
    domain::summaries::{self, DaySummary},
    render::summary::view::{State, View},
    theme::Theme,
};

use super::facts::Reading;

/// Retained navigation and history settings for the Summary panel.
pub struct Summary {
    /// Column selection and scrolling in the shared Summary view.
    pub state: State,
    /// Complete family days before today, matching `h summary`.
    pub complete_days: u32,
    /// Whether history coverage comes from an exported snapshot.
    pub is_offline: bool,
    read_start: Option<f64>,
}

impl Default for Summary {
    fn default() -> Self {
        Self {
            state: State::default(),
            complete_days: crate::config::DEFAULT_DAYS,
            is_offline: false,
            read_start: None,
        }
    }
}

impl Summary {
    /// Aggregates the shared reading without inventing complete days outside its coverage.
    #[must_use]
    pub fn rows(&self, reading: &Reading, now: f64) -> Vec<DaySummary> {
        let mut rows = summaries::build(
            &reading.dataset,
            &reading.calendar,
            reading.rule,
            now,
            self.complete_days as usize + 1,
        );
        let end = reading.dataset.fetched_at;
        let start = self
            .read_start
            .unwrap_or_else(|| self.coverage_start(reading));
        summaries::exclude_incomplete(&mut rows, &reading.calendar, reading.rule, (start, end));
        rows
    }

    /// Retains the successful read's coverage when later settings or refreshes change.
    pub fn record_read(&mut self, reading: &Reading) {
        self.read_start = Some(self.coverage_start(reading));
    }

    fn coverage_start(&self, reading: &Reading) -> f64 {
        let end = reading.dataset.fetched_at;
        if self.is_offline {
            end - reading.dataset.days as f64 * 86400.0
        } else {
            summaries::history_start(
                &reading.calendar,
                reading.rule,
                end,
                u32::try_from(reading.dataset.days).unwrap_or(crate::config::DEFAULT_DAYS),
            )
        }
    }

    /// Draws the same table and chart as `h summary`, retaining its viewport.
    pub fn lines(&mut self, reading: &Reading, size: (u16, u16), now: f64) -> Vec<String> {
        if size.1 < 6 {
            return vec!["Enlarge the panel".to_owned()];
        }
        let rows = self.rows(reading, now);
        let theme = Theme::dark(true);
        let view = View {
            is_hosted: true,
            rows: &rows,
            dataset: &reading.dataset,
            calendar: &reading.calendar,
            units: reading.units,
            now,
        };
        let mut lines = view.lines(&mut self.state, size, theme);
        if let Some(hints) = lines.last_mut() {
            *hints = theme.muted(&crate::prompt::select::clip(
                "←→ h/l a/d columns · H/L visible ends · ↑↓ j/k w/s scroll",
                usize::from(size.0),
            ));
        }
        lines
    }
}

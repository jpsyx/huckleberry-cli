//! One number over time, as a bar chart.
//!
//! Horizontal bars rather than a plotted line, because a terminal row is wide
//! and a terminal cell is coarse: a bar uses the width it has and reads
//! exactly at a glance, where a braille line chart of nine days is a shape
//! rather than a set of numbers.
//!
//! The scale always starts at zero. A chart of daily totals scaled from its
//! own minimum turns eleven, twelve and thirteen wet diapers into a crisis and
//! a recovery, which is the opposite of what this is for.

use crate::cli::{TrendMetric, Units};
use crate::domain::summaries::{self, DaySummary};
use crate::theme::Theme;

use super::format;

/// How wide the bars are drawn.
const BAR_WIDTH: usize = 34;

impl TrendMetric {
    /// What this metric is called on the chart.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Milk => "Milk by bottle",
            Self::Feeds => "Milk feeds",
            Self::Sleep => "Total sleep",
            Self::NightSleep => "Night sleep",
            Self::LongestSleep => "Longest sleep",
            Self::Wet => "Wet diapers",
            Self::Dirty => "Dirty diapers",
            Self::Nursing => "Time nursing",
            Self::Pumped => "Milk expressed",
        }
    }

    /// The figure this metric reads off a day.
    #[must_use]
    pub const fn of(self, row: &DaySummary) -> f64 {
        match self {
            Self::Milk => row.total_ml,
            Self::Feeds => row.feed_count as f64,
            Self::Sleep => row.sleep_seconds,
            Self::NightSleep => row.night_sleep_seconds,
            Self::LongestSleep => row.longest_sleep_seconds,
            Self::Wet => row.wet_count as f64,
            Self::Dirty => row.dirty_count as f64,
            Self::Nursing => row.nursing_seconds,
            Self::Pumped => row.pumped_ml,
        }
    }

    /// How a value of this metric is written.
    #[must_use]
    pub fn render(self, value: f64, units: Units) -> String {
        match self {
            Self::Milk | Self::Pumped => format::volume(Some(value), units),
            Self::Feeds | Self::Wet | Self::Dirty => format!("{value:.0}"),
            Self::Sleep | Self::NightSleep | Self::LongestSleep | Self::Nursing => {
                crate::domain::time::format_duration(value)
            }
        }
    }

    /// The word `--metric` takes, for a chooser to offer.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Milk => "milk",
            Self::Feeds => "feeds",
            Self::Sleep => "sleep",
            Self::NightSleep => "night-sleep",
            Self::LongestSleep => "longest-sleep",
            Self::Wet => "wet",
            Self::Dirty => "dirty",
            Self::Nursing => "nursing",
            Self::Pumped => "pumped",
        }
    }

    /// Every metric, in the order a chooser should offer them.
    pub const ALL: [Self; 9] = [
        Self::Milk,
        Self::Feeds,
        Self::Sleep,
        Self::NightSleep,
        Self::LongestSleep,
        Self::Wet,
        Self::Dirty,
        Self::Nursing,
        Self::Pumped,
    ];

    /// Reads one of those words back.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let wanted = text.trim().to_lowercase();
        Self::ALL
            .into_iter()
            .find(|metric| metric.as_str() == wanted)
    }
}

/// The chart, one line per day, oldest at the top.
///
/// Oldest first is deliberate here and newest first in the log: a chart is
/// read as a shape over time, and time goes downwards.
#[must_use]
pub fn lines(rows: &[DaySummary], metric: TrendMetric, theme: Theme, units: Units) -> Vec<String> {
    let values: Vec<f64> = rows.iter().map(|row| metric.of(row)).collect();
    let peak = values.iter().copied().fold(0.0_f64, f64::max);

    let mut lines = vec![theme.heading(metric.title()), String::new()];
    if peak <= 0.0 {
        lines.push(theme.muted("nothing logged in this window"));
        return lines;
    }

    // Oldest at the top, which is the opposite of the order the summary
    // produces.
    for row in rows.iter().rev() {
        let value = metric.of(row);
        let drawn = format::bar(value, peak, BAR_WIDTH);
        let label = format::pad(&format::day_short(row.day), 11);
        let figure = format::pad_left(&metric.render(value, units), 8);
        let line = format!("{label} {figure} {drawn}");
        // Today is brightest, and still says it is unfinished so its short
        // bar is not read as a drop.
        lines.push(if row.partial {
            format!(
                "{}{}",
                theme.today(&line),
                theme.muted("  (today, still going)")
            )
        } else {
            theme.value(&line)
        });
    }

    lines.push(String::new());
    if let Some(mean) = summaries::average(rows, |row| metric.of(row)) {
        lines.push(theme.muted(&format!(
            "average {} · peak {}",
            metric.render(mean, units),
            metric.render(peak, units)
        )));
    }
    lines.push(theme.muted(&format!(
        "{} {}",
        format::pad("", 11),
        format::sparkline(&values.iter().rev().copied().collect::<Vec<_>>())
    )));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Calendar;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset};

    fn rows(data: &crate::domain::types::Dataset, days: usize) -> Vec<DaySummary> {
        let calendar = Calendar::new("America/New_York").expect("a real timezone");
        summaries::build(data, &calendar, AFTERNOON, days)
    }

    fn chart(data: &crate::domain::types::Dataset, metric: TrendMetric) -> String {
        lines(&rows(data, 3), metric, Theme::dark(false), Units::Ml).join("\n")
    }

    #[test]
    fn an_empty_window_says_so_rather_than_drawing_a_flat_chart() {
        let drawn = chart(&dataset(), TrendMetric::Milk);
        assert!(drawn.contains("nothing logged"), "{drawn}");
        assert!(!drawn.contains('█'), "{drawn}");
    }

    #[test]
    fn the_biggest_day_gets_the_full_bar() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 86_400.0, 300.0),
            bottle(AFTERNOON - 2.0 * 86_400.0, 150.0),
        ];
        let drawn = chart(&data, TrendMetric::Milk);
        let widest = drawn
            .lines()
            .map(|line| line.chars().filter(|cell| *cell == '█').count())
            .max()
            .expect("a line");
        assert_eq!(widest, BAR_WIDTH);
    }

    #[test]
    fn the_chart_runs_oldest_at_the_top() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 86_400.0, 90.0)];
        let drawn = chart(&data, TrendMetric::Feeds);
        let days: Vec<&str> = drawn.lines().filter(|line| line.contains("Sep")).collect();
        assert!(days.first().expect("a row").contains("20 Sep"), "{drawn}");
        assert!(days.last().expect("a row").contains("22 Sep"), "{drawn}");
    }

    #[test]
    fn today_is_marked_as_still_going_so_its_short_bar_is_not_read_as_a_drop() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
        assert!(chart(&data, TrendMetric::Milk).contains("still going"));
    }

    #[test]
    fn a_duration_metric_is_written_as_a_duration_and_a_volume_as_a_volume() {
        assert_eq!(TrendMetric::Sleep.render(8_100.0, Units::Ml), "2h 15m");
        assert_eq!(TrendMetric::Milk.render(90.0, Units::Ml), "90 ml");
        assert_eq!(TrendMetric::Milk.render(88.72, Units::Oz), "3.0 oz");
        assert_eq!(TrendMetric::Wet.render(6.0, Units::Ml), "6");
    }

    #[test]
    fn every_metric_has_a_word_a_title_and_reads_back_from_its_word() {
        for metric in TrendMetric::ALL {
            assert!(!metric.title().is_empty());
            assert_eq!(TrendMetric::parse(metric.as_str()), Some(metric));
        }
        assert_eq!(TrendMetric::parse("nonsense"), None);
    }

    #[test]
    fn a_chart_ends_with_a_sparkline_of_the_same_series() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 86_400.0, 300.0)];
        let drawn = chart(&data, TrendMetric::Milk);
        let last = drawn.lines().last().expect("a line");
        assert_eq!(last.trim().chars().count(), 3, "one cell per day: {last}");
    }
}

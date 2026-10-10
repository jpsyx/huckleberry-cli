//! One column catalog for the table, selection, and graph.

use std::{fmt::Write as _, ops::Range};

use crate::{cli::Units, domain::summaries::DaySummary, render::format};

#[derive(Clone, Copy)]
enum Kind {
    Count,
    Volume,
    Duration,
}

/// A numeric summary column. The day label is deliberately outside this catalog.
pub struct Column {
    /// Umbrella heading spanning adjacent columns.
    pub group: &'static str,
    subgroup: &'static str,
    label: &'static str,
    kind: Kind,
    pick: fn(&DaySummary) -> Option<f64>,
}

impl Column {
    const fn new(
        group: &'static str,
        label: &'static str,
        kind: Kind,
        pick: fn(&DaySummary) -> Option<f64>,
    ) -> Self {
        Self {
            group,
            subgroup: "",
            label,
            kind,
            pick,
        }
    }

    const fn in_subgroup(mut self, subgroup: &'static str) -> Self {
        self.subgroup = subgroup;
        self
    }

    /// Heading in the family's chosen volume unit.
    #[must_use]
    pub fn heading(&self, units: Units) -> String {
        let unit = match units {
            Units::Ml => "ml",
            Units::Oz => "oz",
        };
        match (self.kind, self.label) {
            (Kind::Volume, "avg milk") => format!("milk {unit}/feed"),
            (Kind::Volume, label) => format!("{label} {unit}"),
            (_, label) => label.to_owned(),
        }
    }

    /// Full metric name and unit for the graph.
    #[must_use]
    pub fn title(&self, units: Units) -> String {
        let heading = self.heading(units);
        let suffix = if matches!(self.kind, Kind::Duration) {
            " (hours)"
        } else {
            ""
        };
        format!("{} / {heading}{suffix}", self.group)
    }

    /// Width in terminal cells, including room for selection brackets.
    #[must_use]
    pub fn width(&self, units: Units) -> usize {
        self.heading(units)
            .len()
            .max(if matches!(self.kind, Kind::Duration) {
                8
            } else {
                5
            })
            + 2
    }

    /// Numeric graph value: durations in hours, volumes in the chosen unit.
    #[must_use]
    pub fn value(&self, row: &DaySummary, units: Units) -> Option<f64> {
        let value = (self.pick)(row)?;
        Some(match self.kind {
            Kind::Duration => value / 3600.0,
            Kind::Volume if units == Units::Oz => {
                value / huckleberry_api::models::feed::MILLILITRES_PER_OUNCE
            }
            _ => value,
        })
    }

    fn cell(&self, row: &DaySummary, units: Units) -> String {
        let value = (self.pick)(row).map_or_else(
            || format::MISSING.to_owned(),
            |value| match self.kind {
                Kind::Count => format!("{value:.0}"),
                Kind::Volume => format::volume_bare(Some(value), units),
                Kind::Duration => crate::domain::time::format_duration(value),
            },
        );
        format::pad_left(&value, self.width(units))
    }
}

fn recorded(value: f64) -> Option<f64> {
    (value > 0.0).then_some(value)
}

/// Every selectable metric, in table order.
pub const COLUMNS: &[Column] = &[
    Column::new("feed", "feeds", Kind::Count, |row| {
        recorded(row.feed_count as f64)
    }),
    Column::new("feed", "milk", Kind::Volume, |row| recorded(row.total_ml))
        .in_subgroup("bottle feed"),
    Column::new("feed", "formula", Kind::Volume, |row| {
        recorded(row.formula_ml)
    })
    .in_subgroup("bottle feed"),
    Column::new("feed", "breast", Kind::Volume, |row| {
        recorded(row.breast_milk_ml)
    })
    .in_subgroup("bottle feed"),
    Column::new("feed", "daytime milk", Kind::Volume, |row| {
        recorded(row.day_milk_ml)
    })
    .in_subgroup("bottle feed"),
    Column::new("feed", "night milk", Kind::Volume, |row| {
        recorded(row.night_milk_ml)
    })
    .in_subgroup("bottle feed"),
    Column::new(
        "feed",
        "avg milk",
        Kind::Volume,
        DaySummary::average_milk_ml,
    )
    .in_subgroup("bottle feed"),
    Column::new("feed", "nursed", Kind::Duration, |row| {
        recorded(row.nursing_seconds)
    })
    .in_subgroup("nursed"),
    Column::new("feed", "daytime nursed", Kind::Duration, |row| {
        recorded(row.day_nursing_seconds)
    })
    .in_subgroup("nursed"),
    Column::new("feed", "night nursed", Kind::Duration, |row| {
        recorded(row.night_nursing_seconds)
    })
    .in_subgroup("nursed"),
    Column::new(
        "feed",
        "nurse/feed",
        Kind::Duration,
        DaySummary::average_nursing_seconds,
    )
    .in_subgroup("nursed"),
    Column::new("sleep", "sleep", Kind::Duration, |row| {
        recorded(row.sleep_seconds)
    }),
    Column::new("sleep", "night*", Kind::Duration, |row| {
        recorded(row.night_sleep_seconds)
    }),
    Column::new("sleep", "day", Kind::Duration, |row| {
        recorded(row.day_sleep_seconds)
    }),
    Column::new("sleep", "longest", Kind::Duration, |row| {
        recorded(row.longest_sleep_seconds)
    }),
    Column::new("sleep", "avg sleep", Kind::Duration, |row| {
        row.average_sleep_seconds
    }),
    Column::new("sleep", "avg night sleep", Kind::Duration, |row| {
        row.average_night_sleep_seconds
    }),
    Column::new("sleep", "avg day sleep", Kind::Duration, |row| {
        row.average_nap_seconds
    }),
    Column::new("wake time", "wake time", Kind::Duration, |row| {
        row.wake_seconds
    }),
    Column::new("wake time", "night wake*", Kind::Duration, |row| {
        row.night_wake_seconds
    }),
    Column::new("wake time", "day wake", Kind::Duration, |row| {
        row.day_wake_seconds
    }),
    Column::new("wake time", "avg wake", Kind::Duration, |row| {
        row.average_wake_seconds
    }),
    Column::new("wake time", "avg night wake", Kind::Duration, |row| {
        row.average_night_wake_seconds
    }),
    Column::new("wake time", "avg day wake", Kind::Duration, |row| {
        row.average_day_wake_seconds
    }),
    Column::new("wake time", "longest", Kind::Duration, |row| {
        row.longest_wake_seconds
    }),
    Column::new("diaper", "wet", Kind::Count, |row| {
        recorded(row.wet_count as f64)
    }),
    Column::new("diaper", "dirty", Kind::Count, |row| {
        recorded(row.dirty_count as f64)
    }),
];

/// Leaf headings for the selected visible range.
#[must_use]
pub fn heading_cells(units: Units, range: Range<usize>, selected: Option<usize>) -> Vec<String> {
    range
        .map(|index| {
            let column = &COLUMNS[index];
            let heading = column.heading(units);
            let heading = if selected == Some(index) {
                format!("[{heading}]")
            } else {
                heading
            };
            format::pad_left(&heading, column.width(units))
        })
        .collect()
}

/// Data cells for the same range as the headings.
#[must_use]
pub fn data_cells(row: &DaySummary, units: Units, range: Range<usize>) -> Vec<String> {
    range.map(|index| COLUMNS[index].cell(row, units)).collect()
}

/// A fixed day label and numeric cells, separated at category boundaries.
#[must_use]
pub fn join_cells(day: &str, cells: &[String], range: Range<usize>) -> String {
    let mut line = format::pad(day, 11);
    for (index, cell) in range.zip(cells) {
        let boundary = index == 0 || COLUMNS[index - 1].group != COLUMNS[index].group;
        line.push_str(if boundary { " │ " } else { "  " });
        line.push_str(cell);
    }
    line
}

/// Merged category headings, spanning their visible leaf columns.
#[must_use]
pub fn group_row(units: Units, range: Range<usize>) -> String {
    merged_row(units, range, |column| column.group)
}

/// Merged feeding subcategories; total feeds has no subcategory.
#[must_use]
pub fn subgroup_row(units: Units, range: Range<usize>) -> String {
    merged_row(units, range, |column| column.subgroup)
}

fn merged_row(units: Units, range: Range<usize>, label: fn(&Column) -> &str) -> String {
    let mut line = " ".repeat(11);
    let mut index = range.start;
    while index < range.end {
        let start = index;
        let group = COLUMNS[index].group;
        let heading = label(&COLUMNS[index]);
        while index < range.end
            && COLUMNS[index].group == group
            && label(&COLUMNS[index]) == heading
        {
            index += 1;
        }
        let width = (start..index)
            .map(|column| COLUMNS[column].width(units))
            .sum::<usize>()
            + (index - start - 1) * 2;
        let boundary = start == 0 || COLUMNS[start - 1].group != group;
        line.push_str(if boundary { " │ " } else { "  " });
        let heading: String = heading.chars().take(width).collect();
        let _ = write!(line, "{heading:^width$}");
    }
    line
}

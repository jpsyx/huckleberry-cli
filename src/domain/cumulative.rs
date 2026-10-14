//! Exact cumulative family-day histories and their previous-seven-day average.
//!
//! The x axis is elapsed time from configured day start, never a rolling
//! window. Actual 23/24/25-hour endpoints are retained. Historical averages
//! stop at the shortest contributing day so their denominator stays fixed,
//! and a short day is never projected beyond its real endpoint. Individual
//! curves retain every event and duration boundary; only the historical mean
//! is sampled and interpolated.
//!
//! Today can advance to `now` using an online caller's coverage cutoff, while
//! historical days remain bounded by the dataset's actual fetch time. Offline
//! callers retain that fetch time as their coverage cutoff as well. This keeps
//! a clock tick across day start from completing history that was never read.

mod day;
mod events;
mod interpolation;
mod sleep;

use jiff::civil::Date;

use super::{Calendar, Dataset, today::DayRule};

/// The part of a configured family day contributing to a curve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    /// The whole family day, including its night.
    All,
    /// Day start until night start.
    Day,
    /// Night start until the following day start.
    Night,
}

/// The additive quantity accumulated by a graph, in base units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Metric {
    /// Milk feeds, excluding solids.
    Feeds,
    /// Measured bottle volume in millilitres.
    Milk,
    /// Formula bottle volume in millilitres.
    Formula,
    /// Expressed breast milk volume in millilitres.
    BreastMilk,
    /// Recorded nursing seconds.
    Nursing,
    /// Recorded sleep seconds, counting overlaps once.
    Sleep,
    /// Completed sleeps, counted at their start; live starts do not add a nap.
    Naps,
    /// Elapsed seconds outside recorded sleep.
    Wake,
    /// Wet diaper count.
    Wet,
    /// Dirty diaper count.
    Dirty,
}

/// A cumulative quantity and its day/night restriction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Measure {
    /// The additive quantity.
    pub metric: Metric,
    /// The contributing part of the family day.
    pub period: Period,
}

/// One exact coordinate in elapsed seconds and cumulative base units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Seconds elapsed since this family day's configured start.
    pub seconds: f64,
    /// Accumulated millilitres, seconds, or count.
    pub value: f64,
}

/// An event at its actual time on the cumulative line.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    /// The event's plotted coordinate.
    pub point: Point,
    /// `x` for an event/start, `o` for a completed sleep's end.
    pub symbol: char,
}

/// One individual history, never smoothed or rounded to sampling intervals.
#[derive(Clone, Debug, PartialEq)]
pub struct DayCurve {
    /// The date naming this family day.
    pub day: Date,
    /// Exact straight segments, with duplicate seconds at vertical steps.
    pub points: Vec<Point>,
    /// Actual event times, including starts and completed sleep ends.
    pub markers: Vec<Marker>,
    /// This day's actual length, including daylight-saving transitions.
    pub duration: f64,
    /// Elapsed seconds through the available observation cutoff.
    pub observed: f64,
    /// Whether time or snapshot coverage leaves this day incomplete.
    pub is_partial: bool,
    /// Whether this metric has a relevant recorded observation on this day.
    pub has_data: bool,
}

/// Today, an optional different selected day, and a smooth historical average.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    /// The current discrete family day, clipped to the observation cutoff.
    pub today: DayCurve,
    /// The selected family day, absent when it is today.
    pub selected: Option<DayCurve>,
    /// Shape-preserving interpolation of quarter-hour cumulative means.
    pub average: Vec<Point>,
    /// Covered, recorded days contributing from the previous seven days.
    pub average_days: usize,
}

/// All inputs to the pure cumulative calculation.
pub struct Request<'a> {
    /// The normalized history and current live timers.
    pub dataset: &'a Dataset,
    /// The family's timezone.
    pub calendar: &'a Calendar,
    /// Configured day/night hours; its continuous/discrete mode is ignored.
    pub rule: DayRule,
    /// The observation's current time, in Unix seconds.
    pub now: f64,
    /// The date selected in Summary.
    pub selected: Date,
    /// The additive quantity to graph.
    pub measure: Measure,
    /// Earliest and latest covered Unix timestamps, inclusive. Online callers
    /// may advance the latter to `now`; snapshots keep their actual fetch time.
    pub coverage: (f64, f64),
}

/// Builds histories against configured family days, regardless of totals mode.
#[must_use]
pub fn build(request: &Request<'_>) -> Comparison {
    let today = request.rule.day_of(request.calendar, request.now);
    let history: Vec<_> = (1..=7)
        .map(|back| day::build(request, request.calendar.offset_day(today, -back)))
        .filter(|curve| curve.has_data && !curve.is_partial)
        .collect();
    Comparison {
        today: day::build(request, today),
        selected: (request.selected != today).then(|| day::build(request, request.selected)),
        average: interpolation::average(&history),
        average_days: history.len(),
    }
}

#[cfg(test)]
mod tests;

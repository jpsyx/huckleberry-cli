//! Sleep coverage and the waking gaps between sleeps, using family-day boundaries.

use super::DaySummary;
use crate::domain::{Calendar, today::DayRule, types::Dataset};

type Interval = (f64, f64);

pub(super) fn fill(
    rows: &mut [DaySummary],
    dataset: &Dataset,
    calendar: &Calendar,
    rule: DayRule,
    now: f64,
) {
    let intervals = intervals(dataset, now);
    for row in rows {
        let (start, end) = rule.day_bounds(calendar, row.day);
        let end = end.min(now);
        let night = calendar.at_hour_fraction(row.day, rule.day_end_hour);
        let night = if night < start {
            calendar.at_hour_fraction(calendar.offset_day(row.day, 1), rule.day_end_hour)
        } else {
            night
        };
        row.sleep_seconds = coverage(&intervals, start, end);
        row.night_sleep_seconds = coverage(&intervals, night, end);
        row.day_sleep_seconds = row.sleep_seconds - row.night_sleep_seconds;
        if row.sleep_seconds > 0.0 && !has_unknown_pause(dataset, start, end, now) {
            row.wake_seconds = Some((end - start - row.sleep_seconds).max(0.0));
            row.night_wake_seconds =
                (end > night).then(|| (end - night - row.night_sleep_seconds).max(0.0));
        }
        completed_sleeps(row, dataset, (start, end), night, now);
        waking_gaps(row, &intervals, calendar, rule);
    }
}

fn completed_sleeps(
    row: &mut DaySummary,
    dataset: &Dataset,
    bounds: Interval,
    night_start: f64,
    now: f64,
) {
    let mut naps = Vec::new();
    for sleep in &dataset.sleep {
        if sleep.duration <= 0.0
            || sleep.end() > now
            || sleep.start < bounds.0
            || sleep.start >= bounds.1
        {
            continue;
        }
        row.sleep_count += 1;
        row.longest_sleep_seconds = row.longest_sleep_seconds.max(sleep.duration);
        if sleep.start < night_start {
            naps.push(sleep.duration);
        }
    }
    row.average_nap_seconds = mean(&naps);
}

/// Merge overlapping records before taking their complement or the gaps.
fn intervals(dataset: &Dataset, now: f64) -> Vec<Interval> {
    let mut spans: Vec<_> = dataset
        .sleep
        .iter()
        .map(|sleep| (sleep.start, sleep.end().min(now)))
        .filter(|(start, end)| start.is_finite() && end.is_finite() && end > start)
        .collect();
    if dataset.live.sleep_active
        && let Some(start) = dataset
            .live
            .sleep_start
            .filter(|start| start.is_finite() && *start <= now)
    {
        let end = if dataset.live.sleep_paused {
            dataset.live.sleep_paused_at.unwrap_or(start).min(now)
        } else {
            now
        };
        spans.push((start, end.max(start)));
    }
    spans.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut merged: Vec<Interval> = Vec::new();
    for (start, end) in spans {
        if let Some(previous) = merged.last_mut()
            && start <= previous.1
        {
            previous.1 = previous.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
}

fn coverage(intervals: &[Interval], start: f64, end: f64) -> f64 {
    intervals
        .iter()
        .map(|(began, ended)| (ended.min(end) - began.max(start)).max(0.0))
        .sum()
}

fn waking_gaps(row: &mut DaySummary, intervals: &[Interval], calendar: &Calendar, rule: DayRule) {
    let gaps: Vec<_> = intervals
        .windows(2)
        // A zero-length marker knows when an old paused sleep began, not when it ended.
        .filter(|pair| pair[0].1 > pair[0].0 && rule.day_of(calendar, pair[0].1) == row.day)
        .map(|pair| pair[1].0 - pair[0].1)
        .collect();
    row.average_wake_seconds = mean(&gaps);
    row.longest_wake_seconds = gaps.into_iter().max_by(f64::total_cmp);
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// An old snapshot can say paused without retaining when. That is unknown coverage.
fn has_unknown_pause(dataset: &Dataset, start: f64, end: f64, now: f64) -> bool {
    dataset.live.sleep_active
        && dataset.live.sleep_paused
        && dataset.live.sleep_paused_at.is_none()
        && dataset
            .live
            .sleep_start
            .is_some_and(|began| began < end && now > start)
}

//! Union sleep coverage, its elapsed-time complement, and real event markers.

use super::{
    Metric, Request,
    day::{Observations, Window},
};
use crate::domain::summaries;

pub(super) fn collect(request: &Request<'_>, window: &Window) -> Observations {
    let intervals = summaries::sleep::intervals(request.dataset, window.available_end);
    let has_data = intervals
        .iter()
        .any(|(start, end)| window.overlap(*start, *end))
        || (request.dataset.live.sleep_active
            && request
                .dataset
                .live
                .sleep_start
                .is_some_and(|start| window.contains(start)));
    if request.measure.metric == Metric::Naps {
        return naps(request, window, has_data);
    }
    let mut observations = Observations {
        has_data,
        is_unknown: unknown_pause(request, window),
        ..Observations::default()
    };
    if request.measure.metric == Metric::Wake {
        waking(&mut observations, &intervals, window);
    } else {
        for (start, end) in intervals {
            observations.duration(start, end, window);
        }
    }
    markers(&mut observations, request, window);
    observations
}

fn naps(request: &Request<'_>, window: &Window, has_data: bool) -> Observations {
    let mut observations = Observations {
        has_data,
        ..Observations::default()
    };
    for sleep in &request.dataset.sleep {
        if sleep.duration.is_finite()
            && sleep.duration > 0.0
            && sleep.end() <= request.now.min(request.coverage.1)
            && window.contains(sleep.start)
        {
            observations.has_data = true;
            observations.step(sleep.start, 1.0, window);
        }
    }
    // The count changes at completed sleeps' starts, but both actual endpoints
    // remain visible. A live start is marked at zero without counting a nap.
    observations.markers.clear();
    markers(&mut observations, request, window);
    observations
}

fn waking(observations: &mut Observations, intervals: &[(f64, f64)], window: &Window) {
    let mut cursor = window.available_start;
    for (start, end) in intervals {
        if *end <= cursor {
            continue;
        }
        observations.duration(cursor, *start, window);
        cursor = cursor.max(*end);
    }
    observations.duration(cursor, window.available_end, window);
}

fn markers(observations: &mut Observations, request: &Request<'_>, window: &Window) {
    // Counts belong to their start category; duration markers use actual
    // endpoints within the displayed period, never clipped synthetic ends.
    let counts_starts = request.measure.metric == Metric::Naps;
    let (start_symbol, end_symbol) = if request.measure.metric == Metric::Wake {
        ('x', 'o')
    } else {
        ('o', 'x')
    };
    for sleep in &request.dataset.sleep {
        if !sleep.duration.is_finite()
            || sleep.duration <= 0.0
            || (counts_starts && !window.includes(sleep.start))
        {
            continue;
        }
        observations.marker(sleep.start, start_symbol, window);
        let end = sleep.end();
        if end <= request.now.min(request.coverage.1) {
            observations.marker(end, end_symbol, window);
        }
    }
    if request.dataset.live.sleep_active
        && let Some(start) = request.dataset.live.sleep_start
    {
        observations.marker(start, start_symbol, window);
    }
}

fn unknown_pause(request: &Request<'_>, window: &Window) -> bool {
    let live = &request.dataset.live;
    live.sleep_active
        && live.sleep_paused
        && live.sleep_paused_at.is_none()
        && live
            .sleep_start
            .is_some_and(|start| window.overlap(start, request.now))
}

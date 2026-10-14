//! Observation windows and exact piecewise-linear curve construction.

use jiff::civil::Date;

use super::{DayCurve, Marker, Period, Point, Request, events};

/// Absolute timestamps bounding one family day and its available evidence.
pub(super) struct Window {
    pub start: f64,
    pub end: f64,
    pub available_start: f64,
    pub available_end: f64,
    pub period_start: f64,
    pub period_end: f64,
}

impl Window {
    pub(super) fn create(request: &Request<'_>, day: Date) -> Self {
        let (start, end) = request.rule.day_bounds(request.calendar, day);
        let night = request.rule.night_start(request.calendar, day);
        let (period_start, period_end) = match request.measure.period {
            Period::All => (start, end),
            Period::Day => (start, night),
            Period::Night => (night, end),
        };
        let coverage_end = if day == request.rule.day_of(request.calendar, request.now) {
            request.coverage.1
        } else {
            request.coverage.1.min(request.dataset.fetched_at)
        };
        Self {
            start,
            end,
            available_start: start.max(request.coverage.0).min(end),
            available_end: end.min(request.now).min(coverage_end).max(start),
            period_start,
            period_end,
        }
    }

    pub(super) fn contains(&self, at: f64) -> bool {
        at.is_finite() && at >= self.available_start && at <= self.available_end && at < self.end
    }

    pub(super) fn includes(&self, at: f64) -> bool {
        self.contains(at) && at >= self.period_start && at < self.period_end
    }

    pub(super) fn overlap(&self, start: f64, end: f64) -> bool {
        start.is_finite()
            && end.is_finite()
            && start < self.available_end
            && end > self.available_start
    }

    pub(super) fn clip(&self, start: f64, end: f64) -> Option<(f64, f64)> {
        let start = start.max(self.available_start).max(self.period_start);
        let end = end.min(self.available_end).min(self.period_end);
        (end > start).then_some((start, end))
    }
}

/// A point addition or a duration that increases at a constant rate.
pub(super) struct Contribution {
    pub start: f64,
    pub end: f64,
    pub value: f64,
}

/// Available events and their additive contributions.
#[derive(Default)]
pub(super) struct Observations {
    pub contributions: Vec<Contribution>,
    pub markers: Vec<(f64, char)>,
    pub has_data: bool,
    pub is_unknown: bool,
}

impl Observations {
    pub(super) fn step(&mut self, at: f64, value: f64, window: &Window) {
        if window.includes(at) {
            self.contributions.push(Contribution {
                start: at,
                end: at,
                value,
            });
            self.markers.push((at, 'x'));
        }
    }

    pub(super) fn duration(&mut self, start: f64, end: f64, window: &Window) {
        if let Some((start, end)) = window.clip(start, end) {
            self.contributions.push(Contribution {
                start,
                end,
                value: end - start,
            });
        }
    }
}

pub(super) fn build(request: &Request<'_>, day: Date) -> DayCurve {
    let window = Window::create(request, day);
    let observations = events::collect(request, &window);
    let has_data = observations.has_data && !observations.is_unknown;
    DayCurve {
        day,
        points: if has_data {
            points(&observations, &window)
        } else {
            Vec::new()
        },
        markers: if has_data {
            markers(&observations, &window)
        } else {
            Vec::new()
        },
        duration: window.end - window.start,
        observed: window.available_end - window.start,
        is_partial: window.available_start > window.start
            || window.available_end < window.end
            || observations.is_unknown,
        has_data,
    }
}

fn points(observations: &Observations, window: &Window) -> Vec<Point> {
    let mut times = vec![window.available_start, window.available_end];
    for contribution in &observations.contributions {
        times.extend([contribution.start, contribution.end]);
    }
    times.extend(observations.markers.iter().map(|(at, _)| *at));
    times.sort_by(f64::total_cmp);
    times.dedup();
    let mut points = Vec::new();
    for at in times {
        let before = value_at(observations, at, false);
        let after = value_at(observations, at, true);
        points.push(Point {
            seconds: at - window.start,
            value: before,
        });
        if after > before {
            points.push(Point {
                seconds: at - window.start,
                value: after,
            });
        }
    }
    points
}

fn markers(observations: &Observations, window: &Window) -> Vec<Marker> {
    let mut markers: Vec<_> = observations
        .markers
        .iter()
        .map(|(at, symbol)| Marker {
            point: Point {
                seconds: at - window.start,
                value: value_at(observations, *at, true),
            },
            symbol: *symbol,
        })
        .collect();
    markers.sort_by(|left, right| left.point.seconds.total_cmp(&right.point.seconds));
    markers
}

fn value_at(observations: &Observations, at: f64, include_step: bool) -> f64 {
    observations
        .contributions
        .iter()
        .map(|contribution| {
            if contribution.end > contribution.start {
                let elapsed =
                    (at - contribution.start).clamp(0.0, contribution.end - contribution.start);
                elapsed / (contribution.end - contribution.start) * contribution.value
            } else if at > contribution.start || (include_step && at >= contribution.start) {
                contribution.value
            } else {
                0.0
            }
        })
        .sum()
}

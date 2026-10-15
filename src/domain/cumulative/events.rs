//! Feeding and diaper observations, with missing measurements kept absent.

use super::{
    Metric, Request,
    day::{Observations, Window},
    sleep,
};
use crate::domain::types::FeedEvent;

pub(super) fn collect(request: &Request<'_>, window: &Window) -> Observations {
    match request.measure.metric {
        Metric::Sleep | Metric::Wake | Metric::Naps => sleep::collect(request, window),
        Metric::Wet | Metric::Dirty => diapers(request, window),
        _ => feeds(request, window),
    }
}

fn feeds(request: &Request<'_>, window: &Window) -> Observations {
    let mut observations = Observations::default();
    for feed in &request.dataset.feeds {
        let start = feed.start();
        if request.measure.metric == Metric::Nursing {
            nursing(&mut observations, feed, window);
        } else if window.contains(start)
            && let Some(amount) = amount(feed, request.measure.metric)
        {
            // A measured bottle establishes a known day even for the other
            // milk category, but only a matching bottle is that graph's event.
            observations.has_data = true;
            if matches_bottle_category(feed, request.measure.metric) {
                observations.step(start, amount, window);
            }
        }
    }
    observations
}

fn amount(feed: &FeedEvent, metric: Metric) -> Option<f64> {
    match metric {
        Metric::Feeds => feed.is_milk().then_some(1.0),
        Metric::Milk | Metric::Formula | Metric::BreastMilk => feed
            .millilitres()
            .filter(|amount| amount.is_finite() && *amount >= 0.0),
        _ => None,
    }
}

fn matches_bottle_category(feed: &FeedEvent, metric: Metric) -> bool {
    let is_breast_milk = matches!(
        feed, FeedEvent::Bottle { bottle_type: Some(kind), .. } if kind == "Breast Milk"
    );
    match metric {
        Metric::Formula => !is_breast_milk,
        Metric::BreastMilk => is_breast_milk,
        _ => true,
    }
}

fn nursing(observations: &mut Observations, feed: &FeedEvent, window: &Window) {
    let Some(duration) = feed
        .nursing_seconds()
        .filter(|value| value.is_finite() && *value >= 0.0)
    else {
        return;
    };
    let start = feed.start();
    let end = start + duration;
    if window.overlap(start, end) || window.contains(start) {
        observations.has_data = true;
        observations.duration(start, end, window);
        observations.marker(start, 'o', window);
        if duration > 0.0 {
            observations.marker(end, 'x', window);
        }
    }
}

fn diapers(request: &Request<'_>, window: &Window) -> Observations {
    let mut observations = Observations::default();
    for diaper in &request.dataset.diapers {
        if !window.contains(diaper.start) {
            continue;
        }
        observations.has_data = true;
        let matches = match request.measure.metric {
            Metric::Wet => diaper.wet,
            Metric::Dirty => diaper.dirty,
            _ => false,
        };
        if matches {
            observations.step(diaper.start, 1.0, window);
        }
    }
    observations
}

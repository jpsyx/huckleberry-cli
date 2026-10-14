//! Quarter-hour historical means with shape-preserving cubic interpolation.

use super::{DayCurve, Point};

pub(super) fn average(history: &[DayCurve]) -> Vec<Point> {
    let Some(duration) = history
        .iter()
        .map(|curve| curve.duration)
        .min_by(f64::total_cmp)
    else {
        return Vec::new();
    };
    let mut samples: Vec<_> = (0..(duration / 900.0).ceil() as usize)
        .map(|index| mean(history, index as f64 * 900.0))
        .collect();
    samples.push(mean(history, duration));
    smooth(&samples)
}

fn mean(history: &[DayCurve], seconds: f64) -> Point {
    Point {
        seconds,
        value: history
            .iter()
            .map(|curve| value_at(&curve.points, seconds))
            .sum::<f64>()
            / history.len() as f64,
    }
}

fn value_at(points: &[Point], seconds: f64) -> f64 {
    let after = points.partition_point(|point| point.seconds <= seconds);
    let Some(before) = after.checked_sub(1).and_then(|index| points.get(index)) else {
        return 0.0;
    };
    let Some(following) = points.get(after) else {
        return before.value;
    };
    let fraction = (seconds - before.seconds) / (following.seconds - before.seconds);
    before.value + fraction * (following.value - before.value)
}

/// Harmonic-mean tangents are zero at plateaus and cannot overshoot a sample.
fn smooth(samples: &[Point]) -> Vec<Point> {
    let slopes: Vec<_> = samples
        .windows(2)
        .map(|pair| (pair[1].value - pair[0].value) / (pair[1].seconds - pair[0].seconds))
        .collect();
    let mut dense = Vec::new();
    for (index, pair) in samples.windows(2).enumerate() {
        let left = tangent(samples, &slopes, index);
        let right = tangent(samples, &slopes, index + 1);
        for step in 0..15 {
            let fraction = f64::from(step) / 15.0;
            dense.push(cubic(pair[0], pair[1], left, right, fraction));
        }
    }
    dense.extend(samples.last().copied());
    dense
}

fn tangent(samples: &[Point], slopes: &[f64], index: usize) -> f64 {
    if index == 0 {
        return slopes.first().copied().unwrap_or(0.0);
    }
    let Some(following) = slopes.get(index) else {
        return slopes.last().copied().unwrap_or(0.0);
    };
    let preceding = slopes[index - 1];
    if preceding <= 0.0 || *following <= 0.0 {
        return 0.0;
    }
    let before_width = samples[index].seconds - samples[index - 1].seconds;
    let after_width = samples[index + 1].seconds - samples[index].seconds;
    let before_weight = 2.0 * after_width + before_width;
    let after_weight = after_width + 2.0 * before_width;
    (before_weight + after_weight) / (before_weight / preceding + after_weight / following)
}

fn cubic(start: Point, end: Point, left: f64, right: f64, fraction: f64) -> Point {
    let squared = fraction * fraction;
    let cubed = squared * fraction;
    let width = end.seconds - start.seconds;
    let value = (2.0 * cubed - 3.0 * squared + 1.0) * start.value
        + (cubed - 2.0 * squared + fraction) * width * left
        + (-2.0 * cubed + 3.0 * squared) * end.value
        + (cubed - squared) * width * right;
    Point {
        seconds: start.seconds + fraction * width,
        value: value.clamp(start.value, end.value),
    }
}

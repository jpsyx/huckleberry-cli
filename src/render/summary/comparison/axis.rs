//! Time labels at exact positions, staggered when terminal cells are crowded.

/// Places clock or elapsed-hour labels without overwriting neighboring ticks.
pub(super) fn labels(
    width: usize,
    duration: f64,
    start_hour: f64,
    is_elapsed: bool,
    is_hourly: bool,
) -> Vec<String> {
    let mut rows = Vec::new();
    let mut times = ticks(duration, width, is_hourly);
    // Reserve both endpoints in the first row before placing interior hours.
    times[1..].rotate_right(1);
    for seconds in times {
        let hours = seconds / 3600.0;
        let label = if is_elapsed {
            format!("+{}h", super::plot::quantity(hours))
        } else {
            super::clock_label(start_hour + hours)
        };
        let center = (seconds / duration * (width - 1) as f64).round() as usize;
        place(&mut rows, width, center, &label);
    }
    rows.into_iter()
        .map(|row| row.into_iter().collect())
        .collect()
}

fn ticks(duration: f64, width: usize, is_hourly: bool) -> Vec<f64> {
    if is_hourly {
        (0..(duration / 3600.0).ceil() as usize)
            .map(|hour| hour as f64 * 3600.0)
            .chain(std::iter::once(duration))
            .collect()
    } else {
        let divisions = if width >= 48 { 4 } else { 2 };
        (0..=divisions)
            .map(|index| duration * f64::from(index) / f64::from(divisions))
            .collect()
    }
}

fn place(rows: &mut Vec<Vec<char>>, width: usize, center: usize, label: &str) {
    let start = center
        .saturating_sub(label.len() / 2)
        .min(width - label.len());
    let end = start + label.len();
    let padding = start.saturating_sub(1)..(end + 1).min(width);
    let row = rows
        .iter()
        .position(|row| row[padding.clone()].iter().all(|cell| *cell == ' '))
        .unwrap_or_else(|| {
            rows.push(vec![' '; width]);
            rows.len() - 1
        });
    for (offset, character) in label.chars().enumerate() {
        rows[row][start + offset] = character;
    }
}

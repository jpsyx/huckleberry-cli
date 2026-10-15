//! ASCII cumulative histories, with event markers and a sampled historical mean.

use crate::{
    domain::cumulative::{Comparison, DayCurve},
    theme::{Theme, Tone},
};

mod plot;

/// Render a comparison inside the supplied terminal-cell dimensions.
#[must_use]
pub fn lines(
    comparison: &Comparison,
    title: &str,
    scale: f64,
    start_hour: f64,
    size: (u16, u16),
    theme: Theme,
) -> Vec<String> {
    let (width, height) = (usize::from(size.0), usize::from(size.1));
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let mut output = paint_wrapped(title, Tone::Heading, width, theme);
    output.extend(contents(
        comparison,
        scale,
        start_hour,
        width,
        height.saturating_sub(output.len()),
        theme,
    ));
    output.truncate(height.saturating_sub(1));
    output.resize(height.saturating_sub(1), String::new());
    output.push(paint_clipped("Esc/Enter close", Tone::Muted, width, theme));
    output
}

fn contents(
    comparison: &Comparison,
    scale: f64,
    start_hour: f64,
    width: usize,
    height: usize,
    theme: Theme,
) -> Vec<String> {
    let mut output = Vec::new();
    let legends: Vec<_> = legends(comparison, start_hour)
        .iter()
        .flat_map(|(tone, text)| paint_wrapped(text, *tone, width, theme))
        .collect();
    let notes: Vec<_> = notes(comparison, start_hour)
        .iter()
        .flat_map(|text| paint_wrapped(text, Tone::Muted, width, theme))
        .collect();
    let reserved = 3 + legends.len() + notes.len();
    if width < 28 || height < reserved + 3 {
        output.push(paint_clipped(
            "Enlarge for comparison graph",
            Tone::Muted,
            width,
            theme,
        ));
    } else {
        output.extend(legends);
        if has_data(comparison) {
            let graph = plot::Plot::create(comparison, scale, width, height - reserved);
            output.extend(graph.lines(start_hour, uses_elapsed(comparison), theme));
        } else {
            output.push(paint_clipped(
                "No recorded data for these curves",
                Tone::Muted,
                width,
                theme,
            ));
        }
        output.extend(notes);
    }
    output
}

fn has_data(comparison: &Comparison) -> bool {
    comparison.today.has_data
        || comparison.selected.as_ref().is_some_and(|day| day.has_data)
        || !comparison.average.is_empty()
}

fn legends(comparison: &Comparison, start_hour: f64) -> Vec<(Tone, String)> {
    let cutoff = if uses_elapsed(comparison) {
        format!("{:.1}h elapsed", comparison.today.observed / 3600.0)
    } else {
        clock_label(start_hour + comparison.today.observed / 3600.0)
    };
    let mut result = vec![(
        Tone::Today,
        format!("--- Today (to {cutoff}){}", day_note(&comparison.today)),
    )];
    if let Some(day) = &comparison.selected {
        result.push((
            Tone::Accent,
            format!("- - {} (selected day){}", day.day, day_note(day)),
        ));
    }
    let average = if comparison.average_days == 7 {
        "... 7-day avg".to_owned()
    } else {
        format!("... 7-day avg ({}/7 days)", comparison.average_days)
    };
    let empty = if comparison.average.is_empty() {
        ": no recorded data"
    } else {
        ""
    };
    result.push((Tone::Average, format!("{average}{empty}")));
    result
}

const fn day_note(day: &DayCurve) -> &'static str {
    if !day.has_data {
        ": no recorded data"
    } else if day.is_partial {
        " (partial)"
    } else {
        ""
    }
}

fn notes(comparison: &Comparison, start_hour: f64) -> Vec<String> {
    let mut result = vec![
        "o event/start; x event end; overlapping marks may share a cell".to_owned(),
        "7-day avg: 15-minute samples, smoothed; preceding complete days".to_owned(),
    ];
    if uses_elapsed(comparison) {
        result.push(format!(
            "DST: elapsed hours from {}; clock time may shift",
            clock_label(start_hour)
        ));
    }
    result
}

fn uses_elapsed(comparison: &Comparison) -> bool {
    std::iter::once(&comparison.today)
        .chain(comparison.selected.iter())
        .map(|day| day.duration)
        .chain(comparison.average.last().map(|point| point.seconds))
        .any(|duration| (duration - comparison.clock_duration).abs() > 1.0)
}

fn clock_label(hours: f64) -> String {
    let minutes = (hours * 60.0).round().rem_euclid(1440.0) as u32;
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

fn paint_clipped(text: &str, tone: Tone, width: usize, theme: Theme) -> String {
    let mut used = 0;
    let clipped: String = text
        .chars()
        .take_while(|character| {
            used += ratatui::text::Span::raw(character.to_string()).width();
            used <= width
        })
        .collect();
    theme.paint(tone, &clipped)
}

fn paint_wrapped(text: &str, tone: Tone, width: usize, theme: Theme) -> Vec<String> {
    let mut output = Vec::new();
    let mut line = String::new();
    let mut used = 0;
    for word in text.split_whitespace() {
        let word_width = ratatui::text::Span::raw(word).width();
        if !line.is_empty() && used + 1 + word_width > width {
            output.push(theme.paint(tone, &line));
            line.clear();
            used = 0;
        }
        if !line.is_empty() {
            line.push(' ');
            used += 1;
        }
        for character in word.chars() {
            let cells = ratatui::text::Span::raw(character.to_string()).width();
            if used + cells > width && !line.is_empty() {
                output.push(theme.paint(tone, &line));
                line.clear();
                used = 0;
            }
            if cells <= width {
                line.push(character);
                used += cells;
            }
        }
    }
    if !line.is_empty() {
        output.push(theme.paint(tone, &line));
    }
    output
}

#[cfg(test)]
mod tests;

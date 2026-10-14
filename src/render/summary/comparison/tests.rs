use super::*;
use crate::domain::cumulative::{DayCurve, Marker, Point};

fn day(day: i8, value: f64) -> DayCurve {
    DayCurve {
        day: jiff::civil::date(2026, 10, day),
        points: vec![
            Point {
                seconds: 0.0,
                value: 0.0,
            },
            Point {
                seconds: 7200.0,
                value: 0.0,
            },
            Point {
                seconds: 7200.0,
                value,
            },
            Point {
                seconds: 86400.0,
                value,
            },
        ],
        markers: vec![Marker {
            point: Point {
                seconds: 7200.0,
                value,
            },
            symbol: 'x',
        }],
        duration: 86400.0,
        observed: 86400.0,
        is_partial: false,
        has_data: true,
    }
}

fn comparison() -> Comparison {
    let mut today = day(14, 6.0);
    today.observed = 43200.0;
    today.is_partial = true;
    Comparison {
        today,
        selected: None,
        average: vec![
            Point {
                seconds: 0.0,
                value: 0.0,
            },
            Point {
                seconds: 86400.0,
                value: 4.0,
            },
        ],
        average_days: 3,
    }
}

fn rendered(comparison: &Comparison) -> Vec<String> {
    lines(
        comparison,
        "Feeds (count)",
        1.0,
        6.0,
        (80, 24),
        Theme::dark(false),
    )
}

#[test]
fn today_selection_labels_two_curves_and_the_actual_average_denominator() {
    let text = rendered(&comparison()).join("\n");
    assert!(text.contains("Today"), "{text}");
    assert!(text.contains("7-day avg (3/7"), "{text}");
    assert!(!text.contains("2026-10-14"), "today must not be duplicated");
    assert!(text.contains("15-minute"), "sampling must be explicit");
    assert!(text.contains("06:00") && text.contains("18:00"), "{text}");
    assert!(text.contains("Esc/Enter close"));
}

#[test]
fn another_selection_has_a_dated_legend_and_three_semantic_tones() {
    let mut data = comparison();
    data.selected = Some(day(13, 9.0));
    let output = lines(
        &data,
        "Feeds (count)",
        1.0,
        6.0,
        (80, 24),
        Theme::dark(true),
    );
    let text = output.join("\n");
    assert!(text.contains("2026-10-13"));
    for tone in [
        crate::theme::Tone::Today,
        crate::theme::Tone::Accent,
        crate::theme::Tone::Average,
    ] {
        let style = crate::tui::draw::style(tone);
        assert!(
            output
                .iter()
                .flat_map(|line| crate::tui::draw::painted(line).spans)
                .any(|span| span.style == style)
        );
    }
}

#[test]
fn axis_values_use_display_units() {
    let data = Comparison {
        today: day(14, 7200.0),
        selected: None,
        average: Vec::new(),
        average_days: 0,
    };
    let output = lines(
        &data,
        "Sleep (h)",
        1.0 / 3600.0,
        6.0,
        (80, 24),
        Theme::dark(false),
    );
    assert!(
        output.iter().any(|line| line
            .split('|')
            .next()
            .is_some_and(|label| label.trim() == "2")),
        "{}",
        output.join("\n")
    );
    assert!(!output.join("\n").contains("7200"));
}

#[test]
fn individual_event_and_sleep_end_markers_are_on_the_plot() {
    let mut data = comparison();
    data.today.markers.push(Marker {
        point: Point {
            seconds: 21600.0,
            value: 6.0,
        },
        symbol: 'o',
    });
    let output = rendered(&data);
    let plot: String = output
        .iter()
        .filter_map(|line| line.split_once('|').map(|(_, cells)| cells))
        .collect();
    assert!(plot.contains('x'), "{plot}");
    assert!(plot.contains('o'), "{plot}");
}

#[test]
fn absent_data_is_described_without_drawing_a_measured_zero() {
    let mut data = comparison();
    data.today.has_data = false;
    data.average.clear();
    data.average_days = 0;
    let text = rendered(&data).join("\n");
    assert!(text.contains("no recorded data"), "{text}");
    assert!(text.contains("0/7"), "{text}");
    assert!(!text.lines().any(|line| line.contains('|')), "{text}");
}

#[test]
fn terminal_dimensions_bound_output_and_keep_a_close_hint() {
    for size in [(80, 24), (28, 10), (12, 4), (1, 1), (0, 0)] {
        let output = lines(
            &comparison(),
            "Milk volume (oz)",
            1.0,
            6.0,
            size,
            Theme::dark(true),
        );
        assert!(output.len() <= usize::from(size.1));
        for line in &output {
            assert!(crate::tui::draw::painted(line).width() <= usize::from(size.0));
        }
        if size.0 >= 16 && size.1 >= 4 {
            assert!(
                output
                    .last()
                    .is_some_and(|line| line.contains("Esc/Enter close"))
            );
        }
    }
}

#[test]
fn dst_days_describe_elapsed_time_instead_of_claiming_wrong_clock_ticks() {
    let mut data = comparison();
    data.today.duration = 82800.0;
    let text = rendered(&data).join("\n");
    assert!(text.contains("elapsed") && text.contains("DST"), "{text}");
}

#[test]
fn today_stops_at_the_observation_cutoff_even_if_points_continue() {
    let mut data = comparison();
    data.average.clear();
    data.average_days = 0;
    data.today.observed = 21600.0;
    let output = rendered(&data);
    let rows: Vec<_> = output
        .iter()
        .filter_map(|line| line.split_once('|').map(|(_, cells)| cells))
        .collect();
    assert!(!rows.is_empty());
    for row in rows {
        let cutoff = ((row.len() - 1) as f64 / 4.0).round() as usize;
        assert!(
            row.chars().skip(cutoff + 1).all(|symbol| symbol == ' '),
            "{row}"
        );
    }
}

#[test]
fn exact_steps_have_vertical_strokes_instead_of_diagonal_smoothing() {
    let mut data = comparison();
    data.average.clear();
    data.average_days = 0;
    let output = rendered(&data);
    let plot: String = output
        .iter()
        .filter_map(|line| line.split_once('|').map(|(_, cells)| cells))
        .collect();
    assert!(plot.contains('|'), "{plot}");
    assert!(
        !plot.contains('/'),
        "a count jumps at the event, never before it"
    );
}

#[test]
fn narrow_graphs_wrap_units_and_missing_data_instead_of_hiding_them() {
    let mut data = comparison();
    data.today.has_data = false;
    let output = lines(
        &data,
        "Breast milk cumulative volume (oz)",
        1.0,
        6.0,
        (28, 30),
        Theme::dark(false),
    );
    let text = output.join("\n");
    assert!(text.contains("(oz)"), "{text}");
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(words.contains("no recorded data"), "{text}");
    assert!(text.contains("smoothed"), "{text}");
    assert!(output.iter().all(|line| line.len() <= 28));
}

#[test]
fn dst_average_uses_elapsed_ticks_even_when_today_is_ordinary() {
    let mut data = comparison();
    data.average.last_mut().unwrap().seconds = 90000.0;
    let text = rendered(&data).join("\n");
    assert!(text.contains("DST") && text.contains("+25h"), "{text}");
}

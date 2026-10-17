use super::*;
use crate::domain::{
    fixtures::{AFTERNOON, bottle, dataset, sleep},
    summaries,
    today::DayRule,
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

fn press(state: &mut State, code: KeyCode) -> bool {
    state.apply(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn feed_header_delimiters_follow_visible_subcategory_boundaries() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    let view = View {
        is_hosted: true,
        rows: &[],
        dataset: &data,
        calendar: &calendar,
        units: Units::Oz,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    for (range, expected) in [
        (0..12, vec![12, 22, 130]),
        (1..12, vec![12, 120]),
        (2..12, vec![109]),
        (6..10, vec![46]),
        (7..12, vec![30]),
        (8..12, vec![12]),
        (9..12, vec![]),
    ] {
        for selected in range.clone() {
            let lines = view.head(range.clone(), selected, usize::MAX, Theme::dark(false));
            for line in &lines[2..=3] {
                let separators: Vec<_> = line
                    .chars()
                    .enumerate()
                    .filter_map(|(position, character)| (character == '│').then_some(position))
                    .collect();
                assert_eq!(separators, expected, "{range:?}: {line}");
                assert_eq!(line.chars().count(), table_width(range.clone(), Units::Oz));
            }
        }
    }
}

#[test]
fn all_direction_aliases_stop_at_the_ends_without_closing() {
    for code in [
        KeyCode::Tab,
        KeyCode::Right,
        KeyCode::Char('l'),
        KeyCode::Char('d'),
    ] {
        let mut state = State::default();
        for expected in (1..COLUMNS.len()).chain([COLUMNS.len() - 1]) {
            assert!(!press(&mut state, code));
            assert_eq!(state.selected, expected);
        }
    }
    for code in [
        KeyCode::BackTab,
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('a'),
    ] {
        let mut state = State {
            selected: COLUMNS.len() - 1,
            ..State::default()
        };
        for expected in (0..COLUMNS.len() - 1).rev().chain([0]) {
            assert!(!press(&mut state, code));
            assert_eq!(state.selected, expected);
        }
    }
}

#[test]
fn shifted_tab_moves_back_and_release_events_do_not_move_twice() {
    let mut state = State {
        selected: 1,
        ..State::default()
    };
    assert!(!state.apply(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)));
    assert_eq!(state.selected, 0);
    assert!(!state.apply(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT)));
    assert_eq!(state.selected, 0);
    state.apply(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert_eq!(state.selected, 0);
    assert!(press(&mut state, KeyCode::Esc));
}

#[test]
fn whole_table_shortcuts_select_end_columns_without_changing_vertical_scroll() {
    let mut state = State {
        selected: 4,
        scroll: 3,
        limit: 10,
        ..State::default()
    };
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        for (first, last) in [('g', 'G'), ('0', '$')] {
            state.selected = 4;
            assert!(!state.apply(KeyEvent::new(KeyCode::Char(last), modifiers)));
            assert_eq!(state.selected, COLUMNS.len() - 1);
            assert_eq!(state.scroll, 3);
            assert!(!state.apply(KeyEvent::new(KeyCode::Char(first), modifiers)));
            assert_eq!(state.selected, 0);
            assert_eq!(state.scroll, 3);
        }
    }
    assert!(!press(&mut state, KeyCode::End));
    assert_eq!(state.scroll, 10);
    assert!(!press(&mut state, KeyCode::Home));
    assert_eq!(state.scroll, 0);
}

#[test]
fn every_column_stays_visible_with_day_and_the_correct_graph_when_narrow() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![bottle(AFTERNOON - 3600.0, 90.0)];
    data.sleep = vec![sleep(AFTERNOON - 7200.0, 3600.0)];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 7);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    // The fixed day and bracketed "avg night wake window" need 36 cells.
    for size in [(36, 14), (40, 14), (80, 24), (120, 35), (240, 35)] {
        for (selected, column) in COLUMNS.iter().enumerate() {
            let mut state = State {
                selected,
                ..State::default()
            };
            let lines = view.lines(&mut state, size, Theme::dark(false));
            let drawn = lines.join("\n");
            assert_eq!(lines.len(), usize::from(size.1));
            assert!(
                lines
                    .iter()
                    .all(|line| ratatui::text::Line::raw(line).width() <= usize::from(size.0)),
                "{drawn}"
            );
            assert!(lines[3].contains("day"), "{drawn}");
            assert!(
                lines[3].contains(&format!("[{}]", column.heading(Units::Ml))),
                "{drawn}"
            );
            assert!(drawn.contains("Esc/q back"), "{drawn}");
            assert!(
                drawn.contains(
                    &column
                        .title(Units::Ml)
                        .chars()
                        .take(usize::from(size.0))
                        .collect::<String>()
                ),
                "{drawn}"
            );
            assert!(!drawn.contains('\u{1b}'));
        }
    }
}

#[test]
fn narrow_wake_columns_keep_complete_values_and_bracketed_headings() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    for (seconds, expected_value) in [(2700.0, "45m"), (9000.0, "2h 30m")] {
        let mut rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 1);
        rows[0].average_night_wake_seconds = Some(seconds);
        rows[0].average_day_wake_seconds = Some(seconds);
        let view = View {
            is_hosted: true,
            rows: &rows,
            dataset: &data,
            calendar: &calendar,
            units: Units::Ml,
            now: AFTERNOON,
            coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
        };
        for (heading, shortened) in [
            ("avg night wake window", "[avg night wake w…]"),
            ("avg day wake window", "[avg day wake win…]"),
        ] {
            let mut state = State {
                selected: COLUMNS
                    .iter()
                    .position(|column| column.heading(Units::Ml) == heading)
                    .unwrap(),
                ..State::default()
            };
            let lines = view.lines(&mut state, (32, 14), Theme::dark(false));
            assert!(
                lines[4].ends_with(&format!("[{expected_value}]")),
                "{}",
                lines[4]
            );
            assert!(lines[3].ends_with(shortened), "{}", lines[3]);
            assert_eq!(lines[3].chars().count(), 32);
            assert_eq!(lines[4].chars().count(), 32);
        }
    }
}

#[test]
fn scrolling_reaches_night_hours_and_keeps_the_graph_and_keys() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 30);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let mut state = State::default();
    let mut seen = String::new();
    for _ in 0..80 {
        let lines = view.lines(&mut state, (80, 14), Theme::dark(false));
        let drawn = lines.join("\n");
        assert!(drawn.contains("feed / feeds"));
        assert!(drawn.contains("Esc/q back"));
        seen.push_str(&drawn);
        press(&mut state, KeyCode::Down);
    }
    for text in ["8:00 pm", "7:00 am", "pediatrician"] {
        assert!(seen.contains(text), "missing {text}");
    }
    press(&mut state, KeyCode::Home);
    assert_eq!(state.scroll, 0);
}

#[test]
fn graph_has_chronological_bars_in_display_units_and_missing_day_slots() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    let ounce = huckleberry_api::models::feed::MILLILITRES_PER_OUNCE;
    data.feeds = vec![
        bottle(AFTERNOON, 2.0 * ounce),
        bottle(AFTERNOON - 2.0 * 86400.0, 4.0 * ounce),
    ];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 3);
    let column = COLUMNS
        .iter()
        .find(|column| column.heading(Units::Oz) == "milk oz/feed")
        .unwrap();
    let values = chart::values(&rows, column, Units::Oz);
    assert_eq!(values, vec![Some(4.0), None, Some(2.0)]);
    let lines = chart::lines(&rows, column, Units::Oz, (80, 9), Theme::dark(false)).join("\n");
    assert!(lines.contains("oz/feed"));
    assert!(
        lines.contains("Sep 20") && lines.contains("Sep 22"),
        "{lines}"
    );
    assert!(
        lines
            .chars()
            .any(|character| ('▁'..='█').contains(&character)),
        "{lines}"
    );
}

#[test]
fn a_nine_line_shell_panel_keeps_one_data_row_and_a_graph_visible() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![bottle(AFTERNOON, 90.0)];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 7);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let lines = view.lines(&mut State::default(), (80, 9), Theme::dark(false));
    let drawn = lines.join("\n");
    assert_eq!(lines.len(), 9);
    assert!(drawn.contains("Mon 22 Sep"), "{drawn}");
    assert!(
        drawn
            .chars()
            .any(|character| ('▁'..='█').contains(&character)),
        "{drawn}"
    );
    assert!(drawn.contains("Esc/q back"), "{drawn}");
}

#[test]
fn chart_fills_the_space_after_the_table_and_notes_when_the_panel_grows() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = (0..8)
        .map(|day| bottle(AFTERNOON - f64::from(day) * 86400.0, 90.0))
        .collect();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 8);
    for is_hosted in [true, false] {
        let view = View {
            is_hosted,
            rows: &rows,
            dataset: &data,
            calendar: &calendar,
            units: Units::Ml,
            now: AFTERNOON,
            coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
        };
        let mut plot_heights = Vec::new();
        for height in [40, 60] {
            let lines = view.lines(&mut State::default(), (120, height), Theme::dark(false));
            let chart_start = lines
                .iter()
                .position(|line| line == "feed / feeds")
                .unwrap();
            assert!(
                lines[chart_start - 1].trim().is_empty()
                    && !lines[chart_start - 2].trim().is_empty(),
                "exactly one blank line must separate the notes and chart: {lines:#?}"
            );
            let plot_height = lines[chart_start..]
                .iter()
                .filter(|line| line.contains('│'))
                .count();
            assert!(
                plot_height > 6,
                "the chart must grow past its old six plot rows"
            );
            plot_heights.push(plot_height);
            assert_eq!(lines.len(), usize::from(height));
            let navigation = &lines[lines.len() - if is_hosted { 1 } else { 2 }];
            for hint in [
                "Tab/S-Tab",
                "g/G first/last",
                "Esc/q back",
                "Enter graph",
                "1/1",
            ] {
                assert!(navigation.contains(hint), "{navigation}");
            }
            if is_hosted {
                assert!(!lines.join("\n").contains("as of"));
            } else {
                assert_eq!(lines.last().unwrap(), "as of just now");
            }
        }
        assert_eq!(plot_heights[1] - plot_heights[0], 20);
    }
}

#[test]
fn reversing_inside_the_visible_columns_keeps_the_window_in_place() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 8);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let mut state = State::default();
    for _ in 0..8 {
        let _ = view.lines(&mut state, (100, 24), Theme::dark(false));
        press(&mut state, KeyCode::Right);
    }
    let before = view.lines(&mut state, (100, 24), Theme::dark(false));
    press(&mut state, KeyCode::Left);
    let after = view.lines(&mut state, (100, 24), Theme::dark(false));
    assert_eq!(
        before[1], after[1],
        "reversing within the window must not scroll"
    );
    assert_eq!(
        before[4]
            .replace(['[', ']'], "")
            .split_whitespace()
            .collect::<Vec<_>>(),
        after[4]
            .replace(['[', ']'], "")
            .split_whitespace()
            .collect::<Vec<_>>()
    );
}

#[test]
fn summary_bars_are_separated_and_explain_the_complete_day_average() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    for day in 0..10 {
        data.feeds.push(bottle(
            AFTERNOON - f64::from(day) * 86400.0,
            if day == 0 || day > 7 { 900.0 } else { 100.0 },
        ));
    }
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 10);
    let lines = chart::lines(&rows, &COLUMNS[1], Units::Ml, (100, 12), Theme::dark(false));
    let drawn = lines.join("\n");
    assert!(drawn.contains("··· 7-day avg: 100.0"), "{drawn}");
    assert!(!drawn.contains("today excluded"), "{drawn}");
    assert!(!drawn.contains("/7 days"), "{drawn}");
    assert!(drawn.contains('·'), "the average must be dotted: {drawn}");
    assert!(
        lines.iter().any(|line| line.contains("█ █")),
        "bars need gaps: {drawn}"
    );
}

#[test]
fn age_stays_in_the_header_and_freshness_stays_at_the_bottom_when_scrolling() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.child.birthdate = Some("2025-08-21".into());
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 30);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    for size in [(32, 9), (80, 9), (80, 14), (120, 35)] {
        let mut state = State::default();
        for _ in 0..80 {
            let lines = view.lines(&mut state, size, Theme::dark(false));
            let drawn = lines.join("\n");
            assert_eq!(lines.last().unwrap(), "as of just now");
            assert!(drawn.contains("Esc/q back"), "{drawn}");
            let navigation = &lines[lines.len() - 2];
            assert!(navigation.contains("Esc/q back"), "{drawn}");
            assert!(navigation.contains("g/G"), "{drawn}");
            assert!(
                navigation.contains('↑') && navigation.contains('↓'),
                "{drawn}"
            );
            assert!(navigation.contains(&format!("{}/{}", state.scroll + 1, state.limit + 1)));
            assert_eq!(drawn.matches("as of").count(), 1);
            if size.1 >= 12 {
                assert!(
                    lines[0]
                        .contains("Summary · Bear · 4 weeks 4 days old · column 1/31 · ~ partial")
                );
                assert_eq!(drawn.matches("days old").count(), 1);
            }
            for removed in [
                "Change night",
                "h config set",
                "Wake totals estimate",
                "Rows marked",
                "Averages:",
                "Wake gaps belong",
                "Chart dots:",
                "Feeding day/night splits",
            ] {
                assert!(!drawn.contains(removed), "{drawn}");
            }
            press(&mut state, KeyCode::Down);
        }
    }
}

#[test]
fn unknown_age_is_omitted_from_the_summary_header() {
    let calendar = Calendar::utc();
    let mut data = dataset();
    data.child.birthdate = None;
    let view = View {
        is_hosted: false,
        rows: &[],
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let lines = view.lines(&mut State::default(), (100, 24), Theme::dark(false));
    assert!(lines[0].contains("Summary · Bear · column 1/31"));
    assert!(!lines[0].contains("days old"));
}

#[test]
fn a_panel_too_short_for_any_data_asks_for_more_space() {
    let calendar = Calendar::utc();
    let data = dataset();
    let view = View {
        is_hosted: false,
        rows: &[],
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let lines = view.lines(&mut State::default(), (80, 5), Theme::dark(false));
    assert_eq!(lines.len(), 5);
    assert_eq!(lines[0], "Enlarge the panel");
    assert!(lines[1].contains("Esc/q back"));
}

#[test]
fn visible_edge_shortcuts_keep_the_rendered_window_and_vertical_scroll() {
    let calendar = Calendar::utc();
    let data = dataset();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 30);
    for is_hosted in [true, false] {
        let view = View {
            is_hosted,
            rows: &rows,
            dataset: &data,
            calendar: &calendar,
            units: Units::Ml,
            now: AFTERNOON,
            coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
        };
        let mut state = State {
            selected: 12,
            scroll: 2,
            ..State::default()
        };
        for width in [100, 36, 20, 200, 80] {
            let before = view.lines(&mut state, (width, 24), Theme::dark(false));
            let range = visible_columns(
                state.first_column,
                state.selected,
                Units::Ml,
                usize::from(width),
            );
            let scroll = state.scroll;
            for (key, expected) in visible_edge_keys(range) {
                assert_visible_edge(&view, &mut state, key, expected, width, &before);
                assert_eq!(state.scroll, scroll);
            }
        }
    }
}

fn assert_visible_edge(
    view: &View<'_>,
    state: &mut State,
    key: KeyEvent,
    expected: usize,
    width: u16,
    before: &[String],
) {
    for _ in 0..2 {
        assert!(!state.apply(key));
        assert_eq!(state.selected, expected, "{key:?}, width {width}");
        let after = view.lines(state, (width, 24), Theme::dark(false));
        assert_eq!(before[1..3], after[1..3]);
        assert_eq!(
            before[4]
                .replace(['[', ']'], "")
                .split_whitespace()
                .collect::<Vec<_>>(),
            after[4]
                .replace(['[', ']'], "")
                .split_whitespace()
                .collect::<Vec<_>>()
        );
    }
}

fn visible_edge_keys(range: Range<usize>) -> Vec<(KeyEvent, usize)> {
    let mut keys = Vec::new();
    for modifiers in [KeyModifiers::NONE, KeyModifiers::SHIFT] {
        for (code, expected) in [
            (KeyCode::Char('H'), range.start),
            (KeyCode::Char('L'), range.end - 1),
            (KeyCode::Char('A'), range.start),
            (KeyCode::Char('D'), range.end - 1),
        ] {
            keys.push((KeyEvent::new(code, modifiers), expected));
        }
    }
    keys.extend([
        (
            KeyEvent::new(KeyCode::Left, KeyModifiers::SHIFT),
            range.start,
        ),
        (
            KeyEvent::new(KeyCode::Right, KeyModifiers::SHIFT),
            range.end - 1,
        ),
    ]);
    keys
}

#[test]
fn row_navigation_highlights_one_cell_and_enter_opens_comparison() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![
        bottle(AFTERNOON - 3600.0, 90.0),
        bottle(AFTERNOON - 86400.0, 120.0),
    ];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 3);
    let view = View {
        is_hosted: false,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    for (down, up) in [
        (KeyCode::Down, KeyCode::Up),
        (KeyCode::Char('s'), KeyCode::Char('w')),
        (KeyCode::Char('j'), KeyCode::Char('k')),
    ] {
        let mut state = State {
            selected: 1,
            ..State::default()
        };
        let draw = |state: &mut State| view.lines(state, (100, 24), Theme::dark(false)).join("\n");
        assert!(draw(&mut state).contains("[90]"));
        press(&mut state, down);
        let drawn = draw(&mut state);
        assert!(drawn.contains("[120]"), "{drawn}");
        assert!(
            !drawn.contains("[90]"),
            "only the selected cell is bracketed"
        );
        assert!(
            !drawn.contains("Cumulative"),
            "selection alone does not open a graph"
        );
        press(&mut state, KeyCode::Enter);
        let drawn = draw(&mut state);
        assert!(drawn.contains("Cumulative"), "{drawn}");
        assert!(drawn.contains("2025-09-21"), "{drawn}");
        assert!(
            !press(&mut state, KeyCode::Esc),
            "Escape closes only the comparison"
        );
        assert!(draw(&mut state).contains("[120]"));
        press(&mut state, up);
        assert!(draw(&mut state).contains("[90]"));
    }
}

#[test]
fn shrinking_the_panel_keeps_the_selected_daily_cell_visible() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 30);
    let view = View {
        is_hosted: true,
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
        coverage: (AFTERNOON - 30.0 * 86400.0, AFTERNOON),
    };
    let mut state = State::default();
    let _ = view.lines(&mut state, (100, 60), Theme::dark(false));
    for _ in 0..10 {
        press(&mut state, KeyCode::Down);
    }
    let drawn = view
        .lines(&mut state, (100, 10), Theme::dark(false))
        .join("\n");
    assert!(
        drawn.contains(&super::super::day_label(&rows[10])),
        "{drawn}"
    );
    assert!(
        drawn
            .lines()
            .any(|line| line.contains(&super::super::day_label(&rows[10])) && line.contains("[—]")),
        "{drawn}"
    );
}

#[test]
fn day_and_night_comparisons_label_only_the_configured_clock_hours() {
    use crate::domain::cumulative::{Metric, Period};
    let calendar = Calendar::utc();
    let mut data = dataset();
    let day = "2025-09-22".parse().unwrap();
    data.sleep = vec![sleep(calendar.at(day, 8, 0), 3_600.0)];
    let view = View {
        is_hosted: true,
        rows: &[],
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: calendar.at(day, 23, 0),
        coverage: (f64::NEG_INFINITY, f64::INFINITY),
    };
    for (period, first, last) in [
        (Period::Day, "07:00", "20:00"),
        (Period::Night, "20:00", "07:00"),
    ] {
        let column = COLUMNS
            .iter()
            .position(|column| {
                column.measure().metric == Metric::Sleep && column.measure().period == period
            })
            .unwrap();
        let lines = view.comparison_lines(day, column, (80, 24), Theme::dark(false));
        let text = lines.join("\n");
        let axis = lines
            .iter()
            .position(|line| line.contains('+') && line.contains("---"))
            .unwrap();
        let ticks = lines[axis + 1].trim();
        assert!(ticks.starts_with(first) && ticks.ends_with(last), "{text}");
        assert!(
            !text.contains("DST"),
            "an ordinary partial period is not a DST day: {text}"
        );
    }
}

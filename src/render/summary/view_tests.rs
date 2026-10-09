use super::*;
use crate::domain::{
    fixtures::{AFTERNOON, bottle, dataset, sleep},
    summaries,
    today::DayRule,
};

fn press(state: &mut State, code: KeyCode) -> bool {
    state.apply(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn all_direction_aliases_cycle_every_metric_and_wrap_without_selecting_day() {
    for code in [
        KeyCode::Tab,
        KeyCode::Right,
        KeyCode::Char('l'),
        KeyCode::Char('L'),
        KeyCode::Char('d'),
        KeyCode::Char('D'),
    ] {
        let mut state = State::default();
        for expected in (1..COLUMNS.len()).chain([0]) {
            assert!(!press(&mut state, code));
            assert_eq!(state.selected, expected);
        }
    }
    for code in [
        KeyCode::BackTab,
        KeyCode::Left,
        KeyCode::Char('h'),
        KeyCode::Char('H'),
        KeyCode::Char('a'),
        KeyCode::Char('A'),
    ] {
        let mut state = State::default();
        for expected in (0..COLUMNS.len()).rev() {
            assert!(!press(&mut state, code));
            assert_eq!(state.selected, expected);
        }
    }
}

#[test]
fn shifted_tab_moves_back_and_release_events_do_not_move_twice() {
    let mut state = State::default();
    state.apply(KeyEvent::new(KeyCode::Tab, KeyModifiers::SHIFT));
    assert_eq!(state.selected, 16);
    state.apply(KeyEvent::new_with_kind(
        KeyCode::Tab,
        KeyModifiers::NONE,
        KeyEventKind::Release,
    ));
    assert_eq!(state.selected, 16);
    assert!(press(&mut state, KeyCode::Esc));
}

#[test]
fn every_column_stays_visible_with_day_and_the_correct_graph_when_narrow() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![bottle(AFTERNOON - 3600.0, 90.0)];
    data.sleep = vec![sleep(AFTERNOON - 7200.0, 3600.0)];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 7);
    let view = View {
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
    };
    for size in [(32, 14), (40, 14), (80, 24), (120, 35), (240, 35)] {
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
            assert!(lines[2].contains("day"), "{drawn}");
            assert!(
                lines[2].contains(&format!("[{}]", column.heading(Units::Ml))),
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
fn scrolling_reaches_night_configuration_and_keeps_the_graph_and_keys() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let data = dataset();
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 30);
    let view = View {
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
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
    for text in [
        "h config set day_end 20:00",
        "h config set day_start 07:00",
        "8:00 pm",
        "7:00 am",
        "pediatrician",
    ] {
        assert!(seen.contains(text), "missing {text}");
    }
    press(&mut state, KeyCode::Home);
    assert_eq!(state.scroll, 0);
}

#[test]
fn graph_has_chronological_points_in_display_units_and_breaks_at_missing_days() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    let ounce = huckleberry_api::models::feed::MILLILITRES_PER_OUNCE;
    data.feeds = vec![
        bottle(AFTERNOON, 2.0 * ounce),
        bottle(AFTERNOON - 2.0 * 86400.0, 4.0 * ounce),
    ];
    let rows = summaries::build(&data, &calendar, DayRule::default(), AFTERNOON, 3);
    let segments = chart::segments(&rows, &COLUMNS[4], Units::Oz);
    assert_eq!(segments, vec![vec![(0.0, 4.0)], vec![(2.0, 2.0)]]);
    let lines = chart::lines(&rows, &COLUMNS[4], Units::Oz, (80, 9), Theme::dark(false)).join("\n");
    assert!(lines.contains("oz/feed"));
    assert!(
        lines.contains("Sep 20") && lines.contains("Sep 22"),
        "{lines}"
    );
    assert!(
        lines
            .chars()
            .any(|character| ('\u{2801}'..='\u{28ff}').contains(&character)),
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
        rows: &rows,
        dataset: &data,
        calendar: &calendar,
        units: Units::Ml,
        now: AFTERNOON,
    };
    let lines = view.lines(&mut State::default(), (80, 9), Theme::dark(false));
    let drawn = lines.join("\n");
    assert_eq!(lines.len(), 9);
    assert!(drawn.contains("Mon 22 Sep"), "{drawn}");
    assert!(
        drawn
            .chars()
            .any(|character| ('\u{2801}'..='\u{28ff}').contains(&character)),
        "{drawn}"
    );
    assert!(drawn.contains("Esc/q back"), "{drawn}");
}

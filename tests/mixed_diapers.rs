//! A mixed API entry stays one diaper while contributing to both contents totals.

use app::cli::{TrendMetric, Units};
use app::domain::{
    Calendar, log, normalize, now, stripes, summaries, today::DayRule, types::Dataset,
};
use app::render::{self, summary::columns::COLUMNS};
use app::theme::Theme;
use huckleberry_api::{Located, RowRef};
use serde_json::json;

const NOW: f64 = 1_758_564_000.0;

fn mixed_at(hour: i8, minute: i8) -> (Dataset, Calendar) {
    let calendar = Calendar::new("America/New_York").unwrap();
    let start = calendar.at("2025-09-22".parse().unwrap(), hour, minute);
    let mut data: Dataset = serde_json::from_value(json!({
        "fetched_at": NOW, "timezone": "America/New_York", "days": 3,
        "child": {"cid": "test", "name": "Test", "birthdate": null,
                  "night_start_hour": 20.0, "morning_cutoff_hour": 7.0},
        "growth": null, "sleep": [], "feeds": [], "diapers": [],
        "pumps": [], "milestones": [], "live": {}, "notes": []
    }))
    .unwrap();
    data.diapers = normalize::diapers(&[Located {
        at: RowRef::loose("diaper", "mixed"),
        row: serde_json::from_value(json!({
            "mode": "both", "start": start, "offset": -240,
            "quantity": {"pee": 0, "poo": 100}
        }))
        .unwrap(),
    }]);
    (data, calendar)
}

#[test]
fn early_morning_mixed_counts_on_previous_summary_day_in_both_modes() {
    let (data, calendar) = mixed_at(3, 42);
    for rule in [DayRule::continuous(7.0, 20.0), DayRule::discrete(7.0, 20.0)] {
        let rows = summaries::build(&data, &calendar, rule, NOW, 2);
        assert_eq!(
            (rows[0].wet_count, rows[0].dirty_count, rows[0].diaper_count),
            (0, 0, 0)
        );
        assert_eq!(rows[1].day.to_string(), "2025-09-21");
        assert_eq!(
            (rows[1].wet_count, rows[1].dirty_count, rows[1].diaper_count),
            (1, 1, 1)
        );
        for metric in [TrendMetric::Wet, TrendMetric::Dirty] {
            assert!((metric.of(&rows[1]) - 1.0).abs() < f64::EPSILON);
        }
        for column in COLUMNS.iter().filter(|column| column.group == "diaper") {
            assert_eq!(column.value(&rows[1], Units::Ml), Some(1.0));
        }
    }
}

#[test]
fn now_counts_both_contents_and_labels_the_last_mixed_diaper() {
    let (data, calendar) = mixed_at(3, 42);
    for (rule, expected) in [
        (DayRule::continuous(7.0, 20.0), (1, 1)),
        (DayRule::discrete(7.0, 20.0), (0, 0)),
    ] {
        let view = now::build(&data, &calendar, rule, NOW);
        assert_eq!((view.today.wet, view.today.dirty), expected);
        let text = render::now::lines(
            &view,
            &data,
            &calendar,
            Theme::dark(false),
            Units::Ml,
            NOW,
            Some(120),
        )
        .join("\n");
        assert!(text.contains("wet + dirty"), "{text}");
    }
}

#[test]
fn mixed_is_one_stripe_tick_and_one_calendar_day_log_entry() {
    let (data, calendar) = mixed_at(3, 42);
    let rows = stripes::build(&data, &calendar, DayRule::continuous(7.0, 20.0), NOW, 2);
    assert_eq!(
        rows.iter().map(|row| row.diaper_ticks.len()).sum::<usize>(),
        1
    );
    assert_eq!(rows[0].diaper_ticks.len(), 1);
    let entries = log::build(&data, |_| String::new());
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].description, "mixed · little pee · big poop");
    assert_eq!(calendar.day_of(entries[0].start).to_string(), "2025-09-22");
}

#[test]
fn dashboard_counts_mixed_once_in_total_and_once_in_each_content_column() {
    use app::dashboard::{
        draw,
        state::{State, Tab},
    };
    use ratatui::{Terminal, backend::TestBackend};

    let (data, calendar) = mixed_at(8, 0);
    let mut state = State::new(data, calendar, 2).with_rule(DayRule::continuous(7.0, 20.0));
    state.tab = Tab::Diapers;
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| draw::draw(frame, &state, Units::Ml, NOW))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let rows: Vec<String> = (0..30)
        .map(|y| (0..100).map(|x| buffer[(x, y)].symbol()).collect())
        .collect();
    let row = rows.iter().find(|row| row.contains("Mon 22 Sep")).unwrap();
    let cells: Vec<_> = row.split_whitespace().collect();
    let month_index = cells.iter().position(|cell| *cell == "Sep").unwrap();
    assert_eq!(
        &cells[month_index + 1..month_index + 4],
        &["1", "1", "1"],
        "{row}"
    );
}

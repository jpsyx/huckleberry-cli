//! Volume preferences apply after normalization, wherever history is displayed.

use app::cli::Units;
use app::dashboard::{
    draw,
    state::{State, Tab},
};
use app::domain::{Calendar, log, normalize, types::Dataset};
use app::render;
use app::theme::Theme;
use huckleberry_api::{Located, RowRef};
use ratatui::{Terminal, backend::TestBackend};
use serde_json::json;

const NOW: f64 = 1_758_564_000.0;

fn dataset() -> Dataset {
    let mut data: Dataset = serde_json::from_value(json!({
        "fetched_at": NOW, "timezone": "UTC", "days": 1,
        "child": {"cid": "test", "name": "Test", "birthdate": null,
                  "night_start_hour": 20.0, "morning_cutoff_hour": 7.0},
        "growth": null, "sleep": [], "feeds": [], "diapers": [],
        "pumps": [], "milestones": [], "live": {}, "notes": []
    }))
    .unwrap();
    data.feeds = normalize::feeds(&[
        Located {
            at: RowRef::loose("feed", "ml"),
            row: serde_json::from_value(json!({
                "mode": "bottle", "start": NOW, "amount": 60, "units": "ml",
                "bottleType": "Formula", "offset": 0
            }))
            .unwrap(),
        },
        Located {
            at: RowRef::loose("feed", "oz"),
            row: serde_json::from_value(json!({
                "mode": "bottle", "start": NOW - 60.0, "amount": 1.25, "units": "oz",
                "bottleType": "Breast Milk", "offset": 0
            }))
            .unwrap(),
        },
    ]);
    data.pumps = normalize::pumps(&[Located {
        at: RowRef::loose("pump", "ml"),
        row: serde_json::from_value(json!({
            "start": NOW - 120.0, "entryMode": "leftright", "units": "ml",
            "leftAmount": 30, "rightAmount": 60, "offset": 0, "duration": 900
        }))
        .unwrap(),
    }]);
    data
}

const fn expected(units: Units) -> [&'static str; 3] {
    match units {
        Units::Ml => [
            "60 ml of Formula",
            "37 ml of Breast Milk",
            "90 ml (L 30 ml, R 60 ml) over 15m",
        ],
        Units::Oz => [
            "2.0 oz of Formula",
            "1.2 oz of Breast Milk",
            "3.0 oz (L 1.0 oz, R 2.0 oz) over 15m",
        ],
    }
}

#[test]
fn history_rows_details_and_plain_lines_use_the_display_units() {
    let data = dataset();
    for units in [Units::Oz, Units::Ml] {
        let entries = log::build(&data, |amount| render::format::volume(amount, units));
        let rows = render::log::rows(&entries, &Calendar::utc(), NOW, &|_| None);
        let lines =
            render::log::lines(&entries, &Calendar::utc(), Theme::dark(false), 40, NOW).join("\n");
        for (row, description) in rows.iter().zip(expected(units)) {
            assert_eq!(row.cells[2], description);
            assert!(
                row.detail
                    .iter()
                    .any(|(label, value)| label == "detail" && value == description)
            );
            assert!(lines.contains(description), "{lines}");
        }
    }
}

#[test]
fn dashboard_history_uses_the_display_units() {
    for units in [Units::Oz, Units::Ml] {
        let mut state = State::new(dataset(), Calendar::utc(), 1);
        state.tab = Tab::Log;
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
        terminal
            .draw(|frame| draw::draw(frame, &state, units, NOW))
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text: String = buffer
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect();
        for description in expected(units) {
            assert!(text.contains(description), "{text}");
        }
    }
}

#[test]
fn missing_pump_amounts_stay_distinct_from_recorded_zero() {
    let mut data = dataset();
    let pump = &mut data.pumps[0];
    pump.total_ml = None;
    pump.left_ml = Some(0.0);
    pump.right_ml = None;
    let entries = log::build(&data, |amount| render::format::volume(amount, Units::Oz));
    let rows = render::log::rows(&entries, &Calendar::utc(), NOW, &|_| None);
    let missing = render::format::MISSING;
    assert_eq!(
        rows[2].cells[2],
        format!("{missing} (L 0.0 oz, R {missing}) over 15m")
    );
}

#[test]
fn log_uses_the_configured_default_for_an_offline_snapshot() {
    let directory = std::env::temp_dir().join(format!("h-volume-units-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let snapshot = directory.join("snapshot.json");
    let config = directory.join("config.toml");
    std::fs::write(
        &snapshot,
        app::dataset::render_snapshot(&dataset()).unwrap(),
    )
    .unwrap();
    for units in [Units::Oz, Units::Ml] {
        std::fs::write(&config, format!("units = '{}'\n", units.as_str())).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
            .arg("--config")
            .arg(&config)
            .arg("--offline")
            .arg(&snapshot)
            .arg("log")
            .stdin(std::process::Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let text = String::from_utf8(output.stdout).unwrap();
        for description in expected(units) {
            assert!(text.contains(description), "{text}");
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}

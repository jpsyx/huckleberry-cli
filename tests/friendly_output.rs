//! Human receipts keep units, local dates and readable tables without changing pipes.
use app::domain::Calendar;
use app::render::{format, output};
use app::theme::Theme;

#[test]
fn overnight_sleep_keeps_both_dates_and_a_readable_duration() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let start = calendar.at("2026-09-27".parse().unwrap(), 23, 0);
    let fields = output::sleep_fields(start, 9000.0, &calendar);
    assert_eq!(fields[0].1, "Sep 27, 2026, 11:00 pm EDT");
    assert_eq!(fields[1].1, "Sep 28, 2026, 1:30 am EDT");
    assert_eq!(fields[2].1, "2h 30m");
}

#[test]
fn repeated_wall_clock_times_identify_their_dst_offset() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let first = calendar.at("2026-11-01".parse().unwrap(), 1, 30);
    assert_eq!(
        format::date_time(first, &calendar),
        "Nov 01, 2026, 1:30 am EDT"
    );
    assert_eq!(
        format::date_time(first + 3600.0, &calendar),
        "Nov 01, 2026, 1:30 am EST"
    );
}

#[test]
fn short_sessions_are_not_shown_as_zero_minutes() {
    assert_eq!(output::duration(0.0), "0m");
    assert_eq!(output::duration(35.0), "35s");
    assert_eq!(output::duration(9000.0), "2h 30m");
}

#[test]
fn unicode_names_and_notes_do_not_break_table_alignment() {
    let rows = vec![
        vec!["宝宝 🍼".into(), "90 ml\nA note\tmore".into()],
        vec!["Zoë".into(), "60 ml".into()],
    ];
    let lines = output::table("Bottles", &["Child", "Amount"], &rows, Theme::dark(false));
    let widths: Vec<usize> = lines
        .iter()
        .filter(|line| line.starts_with('│'))
        .map(|line| ratatui::text::Line::raw(line.as_str()).width())
        .collect();
    assert!(widths.iter().all(|width| *width == widths[0]), "{lines:?}");
    assert!(!lines.join("\n").contains('\t'));
    assert!(lines.iter().any(|line| line.contains("90 ml A note more")));
}

#[test]
fn pipes_keep_the_original_machine_payload_exactly() {
    let machine = vec!["start\t1790564400".into(), "duration_seconds\t9000".into()];
    let human = vec!["😴 Sleep recorded".into(), "Duration  2h 30m".into()];
    assert_eq!(output::select(false, human.clone(), &machine), machine);
    assert_eq!(output::select(true, human.clone(), &machine), human);
}

#[test]
fn enum_spellings_become_words() {
    assert_eq!(output::words("breastMilk"), "Breast milk");
    assert_eq!(output::words("wentPotty"), "Went potty");
    assert_eq!(output::words("read-only"), "Read only");
}

#[test]
fn settings_explain_intervals_units_and_missing_values() {
    let fields = output::settings_fields(&app::config::Config::default());
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Refresh interval" && value == "20s")
    );
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Child" && value == "Not selected")
    );
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "History window" && value == "7 days")
    );
}

#[test]
fn narrow_tables_wrap_values_without_losing_text() {
    let rows = vec![vec![
        "宝宝 🍼".into(),
        "A long note about a very sleepy baby".into(),
    ]];
    let lines = output::table_with_width(
        "Bottles",
        &["Child", "Notes"],
        &rows,
        Theme::dark(false),
        28,
    );
    for line in &lines {
        assert!(
            ratatui::text::Line::raw(line.as_str()).width() <= 28,
            "{line}"
        );
    }
    assert!(lines.join(" ").contains("宝宝 🍼"));
    assert!(lines.join(" ").contains("sleepy baby"));
}

#[test]
fn editing_a_sleep_describes_hours_and_minutes() {
    let draft = app::edit::Draft::Sleep(app::edit::SleepDraft {
        minutes: 150.0,
        notes: None,
    });
    assert_eq!(app::edit::summary(&draft), "2h 30m");
}

#[test]
fn editing_a_bottle_keeps_fractional_ounces() {
    let draft = app::edit::Draft::Bottle(app::edit::BottleDraft {
        amount: 1.25,
        kind: app::cli::BottleKind::Formula,
        units: app::cli::Units::Oz,
        notes: None,
    });
    assert_eq!(app::edit::summary(&draft), "1.25 oz of Formula");
}

#[test]
fn paused_sleep_freezes_its_elapsed_time() {
    let timer = serde_json::from_value(serde_json::json!({
        "active": true, "paused": true, "uuid": "test-sleep",
        "timerStartTime": 1_000_000, "timerEndTime": 6_400_000
    }))
    .unwrap();
    let fields = output::sleep_status_fields(Some(&timer), None, 10000.0, &Calendar::utc());
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Status" && value == "Paused")
    );
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Elapsed" && value == "1h 30m")
    );
}

#[test]
fn inactive_sleep_does_not_show_a_stale_start_time() {
    let timer = serde_json::from_value(serde_json::json!({
        "active": false, "paused": false, "uuid": "test-sleep", "timerStartTime": 1_000_000
    }))
    .unwrap();
    let fields = output::sleep_status_fields(Some(&timer), Some(9000.0), 10000.0, &Calendar::utc());
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Status" && value == "Awake")
    );
    assert!(
        fields
            .iter()
            .any(|(label, value)| *label == "Last sleep" && value == "2h 30m")
    );
    assert!(!fields.iter().any(|(label, _)| *label == "Started"));
}

#[test]
fn no_sleep_timer_has_a_readable_status() {
    let fields = output::sleep_status_fields(None, None, 10000.0, &Calendar::utc());
    assert_eq!(fields, vec![("Status", "No sleep in progress".into())]);
}

#[test]
fn narrow_details_use_the_actual_field_label() {
    let lines = output::table_with_width(
        "Settings",
        &["Detail", "Value"],
        &[vec![
            "Measurements".into(),
            "Kilograms and centimetres".into(),
        ]],
        Theme::dark(false),
        28,
    );
    let text = lines.join("\n");
    assert!(text.contains("Measurements: Kilograms"), "{text}");
    assert!(!text.contains("Detail:"), "{text}");
}

#[test]
fn refresh_interval_does_not_discard_seconds() {
    let config = app::config::Config {
        refresh: 90,
        ..Default::default()
    };
    assert!(
        output::settings_fields(&config)
            .iter()
            .any(|(label, value)| *label == "Refresh interval" && value == "90s")
    );
}

#[test]
fn profile_weight_uses_the_configured_units_and_measurement_date() {
    let fields = output::weight_fields(0.453_592_37, 0.0, true, &Calendar::utc());
    assert_eq!(fields[0], ("Weight", "1.00 lb".into()));
    assert!(fields[1].1.contains("Jan 01, 1970"));
    let metric = output::weight_fields(3.6, 0.0, false, &Calendar::utc());
    assert_eq!(metric[0], ("Weight", "3.600 kg".into()));
}

#[test]
fn sleep_start_context_uses_local_time_and_wall_clock_age_even_when_paused() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let start = calendar.at("2026-09-27".parse().unwrap(), 23, 0);
    for paused in [false, true] {
        let timer = serde_json::from_value(serde_json::json!({
            "active": true, "paused": paused, "uuid": "test-sleep",
            "timerStartTime": start * 1000.0, "timerEndTime": (start + 600.0) * 1000.0
        }))
        .unwrap();
        for (elapsed, expected) in [(2400.0, "40m ago"), (9000.0, "2h 30m ago")] {
            let message =
                output::sleep_start_context(Some(&timer), start + elapsed, &calendar).unwrap();
            assert!(message.contains("Sep 27, 2026, 11:00 pm EDT"), "{message}");
            assert!(message.contains(expected), "{message}");
        }
    }
}

#[test]
fn sleep_start_context_omits_inactive_or_missing_starts() {
    assert_eq!(
        output::sleep_start_context(None, 10000.0, &Calendar::utc()),
        None
    );
    for value in [
        serde_json::json!({"active": false, "paused": false, "uuid": "old", "timerStartTime": 1_000_000}),
        serde_json::json!({"active": true, "paused": false, "uuid": "missing"}),
    ] {
        let timer = serde_json::from_value(value).unwrap();
        assert_eq!(
            output::sleep_start_context(Some(&timer), 10000.0, &Calendar::utc()),
            None
        );
    }
}

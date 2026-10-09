use app::domain::{
    Calendar,
    types::{Dataset, FeedEvent},
};
const AFTERNOON: f64 = 1_758_564_000.0;

fn dataset() -> Dataset {
    serde_json::from_value(serde_json::json!({
        "fetched_at": AFTERNOON, "timezone": "America/New_York", "days": 10,
        "child": {"cid": "test", "name": "Test", "birthdate": null,
                  "night_start_hour": 20.0, "morning_cutoff_hour": 7.0},
        "growth": null, "sleep": [], "feeds": [], "diapers": [],
        "pumps": [], "milestones": [], "live": {}, "notes": []
    }))
    .unwrap()
}

fn bottle(start: f64, amount: f64) -> FeedEvent {
    FeedEvent::Bottle {
        at: None,
        id: format!("feed-{start}"),
        start,
        amount_ml: Some(amount),
        bottle_type: Some("Formula".into()),
        notes: None,
    }
}

#[test]
fn summary_defaults_to_today_and_seven_complete_days() {
    let directory = std::env::temp_dir().join(format!("h-summary-window-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let snapshot = directory.join("snapshot.json");
    let config = directory.join("config.toml");
    let mut data = dataset();
    data.fetched_at = huckleberry_api::client::now_seconds();
    std::fs::write(&snapshot, app::dataset::render_snapshot(&data).unwrap()).unwrap();
    std::fs::write(
        &config,
        "day_start = '06:00'\nday_end = '19:30'\nday_mode = 'discrete'\n",
    )
    .unwrap();
    for (args, count) in [(vec![], 8), (vec!["--days", "3"], 4)] {
        let rows = summary_json(&directory, &args);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), count);
        assert_eq!(rows[0]["partial"], true);
        assert!(rows[1..].iter().all(|row| row["partial"] == false));
    }
    data.days = 7;
    std::fs::write(&snapshot, app::dataset::render_snapshot(&data).unwrap()).unwrap();
    let rows = summary_json(&directory, &[]);
    assert_eq!(
        rows[7]["partial"], true,
        "a seven-day export truncates the eighth row"
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn summary_json(directory: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
        .arg("--config")
        .arg(directory.join("config.toml"))
        .arg("--offline")
        .arg(directory.join("snapshot.json"))
        .args(["summary", "--json"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[tokio::test]
async fn shell_read_reloads_day_settings_and_recalculates_existing_history() {
    let directory = std::env::temp_dir().join(format!("h-summary-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let snapshot = directory.join("snapshot.json");
    let config = directory.join("config.toml");
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![
        bottle(calendar.at("2025-09-22".parse().unwrap(), 6, 30), 90.0),
        bottle(calendar.at("2025-09-21".parse().unwrap(), 19, 30), 60.0),
    ];
    std::fs::write(&snapshot, app::dataset::render_snapshot(&data).unwrap()).unwrap();
    let options = app::interactive::session::SessionOptions {
        config: Some(config.clone()),
        offline: Some(snapshot),
        ..Default::default()
    };
    for (start, end, mode, today_milk, night_milk, now_milk) in [
        ("07:00", "20:00", "discrete", 0.0, 90.0, 0.0),
        ("06:00", "19:00", "continuous", 90.0, 60.0, 150.0),
    ] {
        std::fs::write(
            &config,
            format!("day_start = '{start}'\nday_end = '{end}'\nday_mode = '{mode}'\n"),
        )
        .unwrap();
        let (data, calendar, _, rule) =
            app::tui::data::read(&options, app::theme::Theme::dark(false))
                .await
                .unwrap();
        let rows = app::domain::summaries::build(&data, &calendar, rule, AFTERNOON, 2);
        assert!((rows[0].total_ml - today_milk).abs() < f64::EPSILON);
        assert!((rows[1].night_milk_ml - night_milk).abs() < f64::EPSILON);
        let now = app::domain::now::build(&data, &calendar, rule, AFTERNOON);
        assert!((now.today.millilitres - now_milk).abs() < f64::EPSILON);
        assert!((data.child.morning_cutoff_hour - rule.day_start_hour).abs() < f64::EPSILON);
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn retained_shell_facts_use_new_day_hours_even_if_the_network_refresh_fails() {
    let calendar = Calendar::new("America/New_York").unwrap();
    let mut data = dataset();
    data.feeds = vec![bottle(
        calendar.at("2025-09-22".parse().unwrap(), 6, 30),
        90.0,
    )];
    let mut shell = app::tui::App::new();
    shell.refreshed(
        data,
        calendar,
        app::cli::Units::Ml,
        app::domain::today::DayRule::discrete(7.0, 20.0),
    );
    shell.open_dashboard(7);
    shell.apply_day_rule(app::domain::today::DayRule::discrete(6.0, 19.0));
    shell.facts.record_trouble("offline");
    let reading = shell.facts.reading().unwrap();
    let rows = app::domain::summaries::build(
        &reading.dataset,
        &reading.calendar,
        reading.rule,
        AFTERNOON,
        2,
    );
    assert!((rows[0].total_ml - 90.0).abs() < f64::EPSILON);
    assert!((reading.dataset.child.night_start_hour - 19.0).abs() < f64::EPSILON);
    let dashboard = shell.dashboard.as_ref().unwrap();
    let rows = app::domain::summaries::build(
        &dashboard.dataset,
        &dashboard.calendar,
        dashboard.rule,
        AFTERNOON,
        2,
    );
    assert!((rows[0].total_ml - 90.0).abs() < f64::EPSILON);
    assert!((dashboard.dataset.child.night_start_hour - 19.0).abs() < f64::EPSILON);
}

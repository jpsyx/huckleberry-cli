//! All logging commands expose the same flexible time input.
use app::cli::Cli;
use clap::Parser;

#[test]
fn every_recording_and_timer_action_accepts_a_time_flag() {
    for command in [
        "feed bottle",
        "feed solids",
        "diaper",
        "potty",
        "growth",
        "feed nursing pause",
        "feed nursing resume",
        "feed nursing switch",
        "feed nursing stop",
        "sleep pause",
        "sleep resume",
        "sleep stop",
    ] {
        for time in ["now", "358 am", "32 mins ago"] {
            let mut arguments = vec!["hb"];
            arguments.extend(command.split_whitespace());
            arguments.extend(["--at", time]);
            assert!(Cli::try_parse_from(&arguments).is_ok(), "{arguments:?}");
        }
    }
    assert!(
        Cli::try_parse_from(["hb", "feed", "nursing", "start", "--start", "32 mins ago"]).is_ok()
    );
}

fn context() -> app::session::Context {
    app::session::Context {
        config: app::config::Config {
            timezone: "America/New_York".into(),
            ..app::config::Config::default()
        },
        config_path: "unused".into(),
        credentials_path: "unused".into(),
        theme: app::theme::Theme::dark(false),
        verbose: false,
        child_override: None,
        offline: None,
    }
}

#[test]
fn event_times_use_the_sleep_parser_and_noninteractive_default() {
    let context = context();
    let before = huckleberry_api::client::now_seconds();
    let relative = app::prompt::time::read_at(&context, Some("32 mins ago")).unwrap();
    let now = app::prompt::time::read_at(&context, Some("now")).unwrap();
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let default = app::prompt::time::read_at(&context, None).unwrap();
        assert!(default >= before);
    }
    let after = huckleberry_api::client::now_seconds();
    assert!((before - 1920.0..=after - 1920.0).contains(&relative));
    assert!((before..=after).contains(&now));
    for time in ["358 am", "3:58 a.m.", "0358", "03:58"] {
        let event = app::prompt::time::read_at(&context, Some(time)).unwrap();
        let sleep = app::prompt::time::read_start(&context, Some(time)).unwrap();
        assert!((event - sleep).abs() < f64::EPSILON);
    }
    for time in ["nonsense", "32 hours ago", "3:58"] {
        assert!(
            app::prompt::time::read_at(&context, Some(time)).is_err(),
            "{time}"
        );
    }
}

#[test]
fn live_sleep_edits_keep_the_original_instant_and_reuse_relative_parsing() {
    let context = context();
    let original = 1_735_732_800.125;
    let kept = app::prompt::time::read_edit_start(&context, Some("keep"), Some(original)).unwrap();
    assert_eq!(kept, None);
    let before = huckleberry_api::client::now_seconds();
    let changed =
        app::prompt::time::read_edit_start(&context, Some("32 mins ago"), Some(original)).unwrap();
    let after = huckleberry_api::client::now_seconds();
    assert!((before - 1920.0..=after - 1920.0).contains(&changed.unwrap()));
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let error = app::prompt::time::read_edit_start(&context, None, Some(original)).unwrap_err();
        assert!(error.to_string().contains("--set start=<TIME>"));
    }
}

#[test]
fn keeping_a_live_sleep_with_no_start_does_not_invent_one() {
    assert_eq!(
        app::prompt::time::read_edit_start(&context(), Some("keep"), None).unwrap(),
        None
    );
}

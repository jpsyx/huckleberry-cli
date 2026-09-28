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

#[test]
fn history_time_edits_keep_the_exact_existing_instant_and_its_day() {
    let context = context();
    // Jan 1, 2025 at 07:00 in New York, including subsecond precision.
    let original = 1_735_732_800.125;
    let shown = "2025-01-01 7:00 AM";
    let kept = app::prompt::time::read_edit_at(&context, Some(shown), original).unwrap();
    assert_eq!(kept.to_bits(), original.to_bits());
    let changed = app::prompt::time::read_edit_at(&context, Some("8am"), original).unwrap();
    assert_eq!(changed.to_bits(), 1_735_736_400.0_f64.to_bits());
    let another_day =
        app::prompt::time::read_edit_at(&context, Some("2025-01-02 08:00:00"), original).unwrap();
    assert_eq!(another_day.to_bits(), 1_735_822_800.0_f64.to_bits());
    assert!(app::prompt::time::read_edit_at(&context, Some("nonsense"), original).is_err());
}

#[test]
fn history_time_edits_only_use_now_when_explicitly_requested() {
    let context = context();
    let original = 1_735_732_800.125;
    let before = huckleberry_api::client::now_seconds();
    let changed = app::prompt::time::read_edit_at(&context, Some("now"), original).unwrap();
    let after = huckleberry_api::client::now_seconds();
    assert!((before..=after).contains(&changed));
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        assert!(app::prompt::time::read_edit_at(&context, None, original).is_err());
    }
}

#[tokio::test]
async fn every_edit_form_asks_for_time_before_its_details() {
    use app::cli::{BottleKind, DiaperKind, Units};
    use app::edit::{BottleDraft, DiaperDraft, Draft, NursingDraft, SleepDraft, SolidsDraft};
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return;
    }
    let context = context();
    let client = huckleberry_api::Huckleberry::new(
        huckleberry_api::Credentials::new("unused", "unused"),
        "UTC",
    )
    .unwrap();
    let diaper = DiaperDraft {
        potty: false,
        mode: DiaperKind::Pee,
        pee: None,
        poo: None,
        color: None,
        consistency: None,
        rash: false,
        how: None,
        notes: None,
    };
    for mut draft in [
        Some(Draft::Bottle(BottleDraft {
            amount: 90.0,
            units: Units::Ml,
            kind: BottleKind::Formula,
            notes: None,
        })),
        Some(Draft::Diaper(diaper.clone())),
        Some(Draft::Diaper(DiaperDraft {
            potty: true,
            ..diaper
        })),
        Some(Draft::Nursing(NursingDraft {
            left_minutes: 5.0,
            right_minutes: 4.0,
            notes: None,
        })),
        Some(Draft::Solids(SolidsDraft {
            foods: vec!["Avocado".into()],
            amount: "some".into(),
            reaction: None,
            notes: None,
        })),
        Some(Draft::Sleep(SleepDraft {
            minutes: 30.0,
            notes: None,
        })),
        None,
    ] {
        let mut started = 1000.0;
        let error =
            app::commands::edit::form::fill(&context, &client, "child", &mut draft, &mut started)
                .await
                .unwrap_err();
        assert!(error.to_string().contains("--set at="), "{error}");
        assert_eq!(started.to_bits(), 1000.0_f64.to_bits());
    }
}

#[test]
fn invalid_history_times_are_rejected_before_saving_other_fields() {
    let context = context();
    for input in ["2099-01-01 08:00", "1960-01-01 08:00"] {
        let error = app::prompt::time::read_edit_at(&context, Some(input), 1_735_732_800.0);
        assert!(
            error.is_err(),
            "{input} must be refused before detail editing"
        );
    }
}

#[test]
fn history_time_defaults_are_shown_and_read_as_a_clock_with_am_or_pm() {
    let context = context();
    // Jan 1, 2025 at 07:00 in New York, including subsecond precision.
    let original = 1_735_732_800.125;
    let evening =
        app::prompt::time::read_edit_at(&context, Some("2025-01-02 8:00 PM"), original).unwrap();
    assert_eq!(evening.to_bits(), 1_735_866_000.0_f64.to_bits());
    let padded =
        app::prompt::time::read_edit_at(&context, Some("2025-01-02 08:00 am"), original).unwrap();
    assert_eq!(padded.to_bits(), 1_735_822_800.0_f64.to_bits());
}

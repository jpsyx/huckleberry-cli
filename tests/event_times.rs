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
        "sleep end",
    ] {
        for time in ["now", "358 am", "32 mins ago"] {
            let mut arguments = vec!["h"];
            arguments.extend(command.split_whitespace());
            arguments.extend(["--at", time]);
            assert!(Cli::try_parse_from(&arguments).is_ok(), "{arguments:?}");
        }
    }
    assert!(
        Cli::try_parse_from(["h", "feed", "nursing", "start", "--start", "32 mins ago"]).is_ok()
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
fn all_time_readers_accept_the_same_relative_inputs_without_losing_the_date() {
    let context = context();
    let original = 1_735_732_800.125;
    for (input, seconds) in [
        ("32 min ago", 1920.0),
        ("40 minutes ago.", 2400.0),
        ("3000m ago", 180_000.0),
        ("now", 0.0),
        ("right now!", 0.0),
    ] {
        let before = huckleberry_api::client::now_seconds();
        let results = [
            app::prompt::time::read_at(&context, Some(input)),
            app::prompt::time::read_start(&context, Some(input)),
            app::prompt::time::read_end(&context, Some(input)),
            app::prompt::time::read_manual_start(&context, Some(input)),
            app::prompt::time::read_manual_end(&context, Some(input), original),
            app::prompt::time::read_edit_at(&context, Some(input), original),
            app::prompt::time::read_edit_stop(&context, Some(input), original, original + 3600.0),
            app::prompt::time::read_edit_start(&context, Some(input), Some(original))
                .map(Option::unwrap),
            app::prompt::time::read_edit_start(&context, Some(input), None).map(Option::unwrap),
        ];
        let after = huckleberry_api::client::now_seconds();
        for result in results {
            let instant = result.unwrap();
            assert!(
                (before - seconds..=after - seconds).contains(&instant),
                "{input}"
            );
        }
    }
}

#[test]
fn manual_sleep_can_mix_clock_and_relative_inputs() {
    let context = context();
    let calendar = context.calendar().unwrap();
    let now = huckleberry_api::client::now_seconds();
    let clock_start = calendar.zoned(now - 7200.0).strftime("%I:%M%p").to_string();
    let started = app::prompt::time::read_manual_start(&context, Some(&clock_start)).unwrap();
    let ended = app::prompt::time::read_manual_end(&context, Some("now"), started).unwrap();
    assert!((now..=huckleberry_api::client::now_seconds()).contains(&ended));
    assert!(ended > started);

    let started = app::prompt::time::read_manual_start(&context, Some("180m ago")).unwrap();
    let clock_end = calendar.zoned(now - 3600.0).strftime("%I:%M%p").to_string();
    let ended = app::prompt::time::read_manual_end(&context, Some(&clock_end), started).unwrap();
    assert_eq!(
        calendar.zoned(ended).strftime("%I:%M%p").to_string(),
        clock_end
    );
    assert!(ended > started);
}

#[test]
fn manual_clock_end_uses_the_first_occurrence_after_a_relative_start_in_a_repeated_hour() {
    let context = context();
    let now = "2025-11-02T02:30:00-05:00"
        .parse::<jiff::Timestamp>()
        .unwrap()
        .as_second() as f64;
    for input in ["60 min ago", "120 min ago"] {
        let started = app::domain::clock::parse_relative(input, now).unwrap();
        let ended = app::prompt::time::read_manual_end(&context, Some("1:45am"), started).unwrap();
        assert_eq!((ended - started).to_bits(), 900.0_f64.to_bits(), "{input}");
        assert!(ended < now);
    }
}

#[test]
fn manual_sleep_keeps_overnight_clock_resolution() {
    let context = context();
    let started = app::prompt::time::read_manual_start(&context, Some("11:30pm")).unwrap();
    let ended = app::prompt::time::read_manual_end(&context, Some("01:15"), started).unwrap();
    assert_eq!(context.calendar().unwrap().zoned(ended).hour(), 1);
    assert_eq!(context.calendar().unwrap().zoned(ended).minute(), 15);
    assert!(ended > started);
}

#[test]
fn manual_sleep_still_requires_both_times_without_a_terminal() {
    if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        return;
    }
    let context = context();
    let error = app::prompt::time::read_manual_start(&context, None).unwrap_err();
    assert!(error.to_string().contains("--start <TIME>"));
    let error = app::prompt::time::read_manual_end(&context, None, 1000.0).unwrap_err();
    assert!(error.to_string().contains("--end <TIME>"));
}

#[test]
fn manual_sleep_rejects_invalid_inputs_and_ambiguous_flags() {
    let context = context();
    for input in ["-40 min ago", "1.5 min ago", "nope", "3:58"] {
        assert!(app::prompt::time::read_manual_start(&context, Some(input)).is_err());
        assert!(app::prompt::time::read_manual_end(&context, Some(input), 1000.0).is_err());
    }
}

#[test]
fn all_time_readers_reject_relative_times_before_the_unix_epoch() {
    let context = context();
    let input = Some("4294967295m ago");
    let original = 1_735_732_800.125;
    for result in [
        app::prompt::time::read_at(&context, input),
        app::prompt::time::read_start(&context, input),
        app::prompt::time::read_end(&context, input),
        app::prompt::time::read_manual_start(&context, input),
        app::prompt::time::read_manual_end(&context, input, original),
        app::prompt::time::read_edit_at(&context, input, original),
        app::prompt::time::read_edit_stop(&context, input, original, original + 3600.0),
        app::prompt::time::read_edit_start(&context, input, Some(original)).map(Option::unwrap),
        app::prompt::time::read_edit_start(&context, input, None).map(Option::unwrap),
    ] {
        assert!(result.is_err(), "pre-epoch input must fail: {result:?}");
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

#[test]
fn sleep_end_is_an_alias_for_stop_with_or_without_an_explicit_time() {
    for extra in [vec![], vec!["--at", "32 min ago"]] {
        let mut end = vec!["h", "sleep", "end"];
        let mut stop = vec!["h", "sleep", "stop"];
        end.extend(&extra);
        stop.extend(&extra);
        assert_eq!(
            Cli::try_parse_from(end).unwrap().command,
            Cli::try_parse_from(stop).unwrap().command
        );
    }
}

#[test]
fn sleep_end_reuses_start_time_parsing_and_defaults() {
    let context = context();
    for input in ["1:23 pm", "123pm", "0358", "21:30"] {
        assert_eq!(
            app::prompt::time::read_end(&context, Some(input))
                .unwrap()
                .to_bits(),
            app::prompt::time::read_start(&context, Some(input))
                .unwrap()
                .to_bits()
        );
    }
    let before = huckleberry_api::client::now_seconds();
    let relative = app::prompt::time::read_end(&context, Some("32 min ago")).unwrap();
    let now = app::prompt::time::read_end(&context, Some("now")).unwrap();
    let after = huckleberry_api::client::now_seconds();
    assert!((before - 1920.0..=after - 1920.0).contains(&relative));
    assert!((before..=after).contains(&now));
    if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        let before = huckleberry_api::client::now_seconds();
        let default = app::prompt::time::read_end(&context, None).unwrap();
        assert!((before..=huckleberry_api::client::now_seconds()).contains(&default));
    }
    for input in ["nonsense", "32 hours ago", "1:23"] {
        assert!(app::prompt::time::read_end(&context, Some(input)).is_err());
    }
}

#[test]
fn event_times_accept_trailing_sentence_punctuation() {
    let context = context();
    for input in [
        "40 minutes ago.",
        "40 minutes ago!",
        " 40 minutes ago, ",
        "40 minutes ago?;",
        "40 minutes ago…",
    ] {
        let before = huckleberry_api::client::now_seconds();
        let parsed = app::prompt::time::read_end(&context, Some(input)).unwrap();
        let after = huckleberry_api::client::now_seconds();
        assert!(
            (before - 2400.0..=after - 2400.0).contains(&parsed),
            "{input}"
        );
    }
    for input in ["now.", "right now!"] {
        let before = huckleberry_api::client::now_seconds();
        let parsed = app::prompt::time::read_start(&context, Some(input)).unwrap();
        assert!((before..=huckleberry_api::client::now_seconds()).contains(&parsed));
    }
    for input in ["1:23 pm!", "1:23 pm,"] {
        let parsed = app::prompt::time::read_end(&context, Some(input)).unwrap();
        assert_eq!(
            parsed.to_bits(),
            app::prompt::time::read_end(&context, Some("1:23 pm"))
                .unwrap()
                .to_bits()
        );
    }
}

#[test]
fn punctuation_does_not_hide_invalid_relative_durations() {
    for input in [
        "-40 minutes ago.",
        "1.5 minutes ago.",
        "4,0 minutes ago.",
        "40 minutes ago extra.",
        "40 minutes ago:",
    ] {
        assert!(
            app::prompt::time::read_end(&context(), Some(input)).is_err(),
            "{input}"
        );
    }
}

#[test]
fn a_sleeps_stop_is_read_as_a_time_and_keeps_its_exact_instant() {
    let context = context();
    // Jan 1, 2025 at 07:00 in New York, including subsecond precision.
    let started = 1_735_732_800.125;
    let stopped = started + 5400.0;
    let shown = "2025-01-01 8:30 AM";
    for kept in [shown, "keep"] {
        assert_eq!(
            app::prompt::time::read_edit_stop(&context, Some(kept), started, stopped)
                .unwrap()
                .to_bits(),
            stopped.to_bits(),
            "{kept}"
        );
    }
    let clock =
        app::prompt::time::read_edit_stop(&context, Some("9:15am"), started, stopped).unwrap();
    assert_eq!(clock.to_bits(), 1_735_740_900.0_f64.to_bits());
    let dated =
        app::prompt::time::read_edit_stop(&context, Some("2025-01-02 08:00:00"), started, stopped)
            .unwrap();
    assert_eq!(dated.to_bits(), 1_735_822_800.0_f64.to_bits());
    assert!(
        app::prompt::time::read_edit_stop(&context, Some("nonsense"), started, stopped).is_err()
    );
}

#[test]
fn a_sleeps_stop_must_come_after_its_start() {
    let context = context();
    let started = 1_735_732_800.125;
    for refused in ["2025-01-01 6:00 AM", "2025-01-01 7:00 AM"] {
        let failure =
            app::prompt::time::read_edit_stop(&context, Some(refused), started, started + 5400.0)
                .expect_err("a sleep ends after it starts");
        assert!(failure.to_string().contains("after"), "{failure}");
    }
    // A clock-only answer earlier in the day means the next day, not a refusal.
    let overnight =
        app::prompt::time::read_edit_stop(&context, Some("6:00am"), started, started + 5400.0)
            .unwrap();
    assert!(overnight > started);
}

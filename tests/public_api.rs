//! The library surface, exercised from outside the crate.
//!
//! The binary is a shell around the library, so these tests drive the library
//! rather than spawning a process. That is also a check that the surface is
//! usable from outside: anything these cannot reach is something `main.rs`
//! should not be reaching either.
//!
//! Nothing here touches the network. The screens are driven from a snapshot,
//! which is the same path `--offline` takes.

use app::cli::{Cli, Command, ConfigAction, DiaperKind, SleepAction, Units};
use app::config::{self, Config};
use app::credentials::{self, Resolved, Stored};
use app::domain::types::Dataset;
use app::domain::{Calendar, log, now, stripes, summaries};
use app::prompt::{Question, Reply, interpret, unanswerable_message};
use app::render;
use app::theme::{Theme, Tone};
use clap::{CommandFactory, Parser};

/// A week of a three-week-old, ending at a fixed afternoon, as JSON.
///
/// Written out rather than built from the domain types on purpose: this is the
/// file format other people's tools will write, and a change that breaks
/// reading it should break this test.
fn snapshot_json() -> String {
    let now = 1_758_564_000.0_f64; // 2025-09-22T18:00:00Z, 2pm in New York.
    let day = 86_400.0;
    let mut feeds = Vec::new();
    let mut diapers = Vec::new();
    let mut sleeps = Vec::new();
    for back in 0..7_i32 {
        let base = now - f64::from(back) * day;
        for (index, hours) in [2.0, 5.0, 8.0, 11.0].into_iter().enumerate() {
            let at = base - hours * 3600.0;
            feeds.push(format!(
                r#"{{"kind":"bottle","id":"f{back}-{index}","start":{at},"amount_ml":90,
                   "bottle_type":"Formula","notes":null}}"#
            ));
            diapers.push(format!(
                r#"{{"id":"d{back}-{index}","start":{at},"mode":"pee","wet":true,"dirty":false,
                   "pee_size":null,"poo_size":null,"color":null,"consistency":null,
                   "rash":false,"potty":false,"notes":null}}"#
            ));
        }
        sleeps.push(format!(
            r#"{{"id":"s{back}","start":{},"duration":10800,"locations":["onOwnInBed"],
               "start_mood":[],"end_mood":[],"notes":null}}"#,
            base - 20.0 * 3600.0
        ));
    }
    format!(
        r#"{{"version":1,"fetched_at":{now},"timezone":"America/New_York","days":7,
           "child":{{"cid":"c1","name":"Wren","birthdate":"2025-09-01",
                     "night_start_hour":20.0,"morning_cutoff_hour":6.75}},
           "growth":null,
           "sleep":[{}],"feeds":[{}],"diapers":[{}],"pumps":[],"milestones":[],
           "live":{{}},"notes":[]}}"#,
        sleeps.join(","),
        feeds.join(","),
        diapers.join(",")
    )
}

fn dataset() -> Dataset {
    app::dataset::parse_snapshot(&snapshot_json()).expect("the snapshot parses")
}

/// The fixed instant the snapshot is written against.
const AFTERNOON: f64 = 1_758_564_000.0;

#[test]
fn the_command_line_surface_is_well_formed() {
    Cli::command().debug_assert();
}

#[test]
fn every_action_is_reachable_without_a_prompt() {
    let diaper = Cli::try_parse_from(["huckleberry-cli", "diaper", "--mode", "both"])
        .expect("a diaper parses");
    assert!(matches!(
        diaper.command.unwrap(),
        Command::Diaper {
            mode: Some(DiaperKind::Both),
            ..
        }
    ));

    let stop = Cli::try_parse_from(["huckleberry-cli", "sleep", "stop"]).expect("sleep parses");
    assert_eq!(
        stop.command.unwrap(),
        Command::Sleep {
            action: SleepAction::Stop { at: None }
        }
    );

    let set = Cli::try_parse_from(["huckleberry-cli", "config", "set", "days", "14"])
        .expect("config set parses");
    assert_eq!(
        set.command.unwrap(),
        Command::Config {
            action: ConfigAction::Set {
                key: Some("days".to_owned()),
                value: Some("14".to_owned()),
            }
        }
    );
}

#[test]
fn a_snapshot_round_trips_through_its_file_format() {
    let dataset = dataset();
    let text = app::dataset::render_snapshot(&dataset).expect("serializing");
    assert_eq!(
        app::dataset::parse_snapshot(&text).expect("parsing"),
        dataset
    );
}

#[test]
fn the_now_screen_answers_the_question_it_exists_for() {
    let dataset = dataset();
    let calendar = Calendar::new(&dataset.timezone).expect("a real timezone");
    let view = now::build(
        &dataset,
        &calendar,
        app::domain::today::DayRule::discrete(6.0, None),
        AFTERNOON,
    );
    let screen = render::now::lines(
        &view,
        &dataset,
        &calendar,
        Theme::dark(false),
        Units::Ml,
        AFTERNOON,
    )
    .join("\n");

    assert!(screen.contains("Wren"), "{screen}");
    assert!(screen.contains("Last fed"), "{screen}");
    assert!(screen.contains("2h 0m ago"), "{screen}");
    assert!(screen.contains("90 ml of Formula"), "{screen}");
    assert!(screen.contains("as of"), "{screen}");
}

#[test]
fn the_day_table_has_one_row_per_day_and_they_all_line_up() {
    let dataset = dataset();
    let calendar = Calendar::new(&dataset.timezone).expect("a real timezone");
    let rows = summaries::build(&dataset, &calendar, AFTERNOON, 7);
    assert_eq!(rows.len(), 7);

    let width = render::summary::heading_row(Units::Ml).chars().count();
    for row in &rows {
        assert_eq!(
            render::summary::data_row(row, Units::Ml).chars().count(),
            width,
            "the row for {} is a different width from the heading",
            row.day
        );
    }
}

#[test]
fn the_stripe_chart_covers_every_day_at_a_fixed_width() {
    let dataset = dataset();
    let calendar = Calendar::new(&dataset.timezone).expect("a real timezone");
    for row in stripes::build(&dataset, &calendar, AFTERNOON, 7) {
        assert_eq!(render::stripes::cells(&row).len(), render::stripes::CELLS);
    }
}

#[test]
fn the_log_loses_nothing() {
    let dataset = dataset();
    let expected = dataset.sleep.len() + dataset.feeds.len() + dataset.diapers.len();
    assert_eq!(
        log::build(&dataset, |amount| render::format::volume(amount, Units::Ml)).len(),
        expected
    );
}

#[test]
fn the_configuration_round_trips_through_its_file_format() {
    let mut settings = Config::default();
    settings.set("child", "c1").expect("a child id");
    settings
        .set("timezone", "America/New_York")
        .expect("a timezone");
    let text = config::serialize(&settings).expect("serializing");
    assert_eq!(config::parse(&text).expect("parsing"), settings);
}

#[test]
fn no_secret_can_be_stored_in_the_settings_file() {
    for forbidden in ["password", "email", "token", "session"] {
        assert!(
            !Config::KEYS.contains(&forbidden),
            "`{forbidden}` must not be a setting"
        );
        assert!(Config::default().set(forbidden, "x").is_err());
    }
}

#[test]
fn a_password_from_the_environment_never_reaches_the_credentials_file() {
    let stored = Stored {
        email: "parent@example.com".to_owned(),
        password: None,
        session: None,
    };
    let resolved = credentials::resolve(&stored, None, Some("from-the-env"));
    assert_eq!(resolved.password.as_deref(), Some("from-the-env"));
    assert_eq!(credentials::to_store(&resolved).password, None);
}

#[test]
fn a_typed_password_is_kept_so_nobody_is_asked_twice() {
    let resolved = Resolved {
        email: Some("parent@example.com".to_owned()),
        password: Some("typed".to_owned()),
        password_from_environment: false,
        session: None,
    };
    assert_eq!(
        credentials::to_store(&resolved).password.as_deref(),
        Some("typed")
    );
}

#[test]
fn an_omitted_value_is_asked_for_only_when_there_is_somebody_to_ask() {
    let question = Question::new("amount", "How much?", "--amount <NUMBER>");
    assert!(unanswerable_message(&question).contains("--amount <NUMBER>"));
    assert_eq!(
        interpret(" 90 \n", question.choices, question.default),
        Reply::Accepted("90".to_owned())
    );
}

#[test]
fn piped_output_carries_no_escape_sequences() {
    let plain = Theme::dark(false);
    assert_eq!(plain.paint(Tone::Success, "done"), "done");

    let dataset = dataset();
    let calendar = Calendar::new(&dataset.timezone).expect("a real timezone");
    let view = now::build(
        &dataset,
        &calendar,
        app::domain::today::DayRule::discrete(6.0, None),
        AFTERNOON,
    );
    let screen =
        render::now::lines(&view, &dataset, &calendar, plain, Units::Ml, AFTERNOON).join("\n");
    assert!(!screen.contains('\u{1b}'), "{screen}");
}

#[test]
fn nothing_a_baby_does_is_ever_an_error_or_a_warning() {
    let dataset = dataset();
    let calendar = Calendar::new(&dataset.timezone).expect("a real timezone");
    let rows = summaries::build(&dataset, &calendar, AFTERNOON, 7);
    let painted = render::summary::lines(
        &rows,
        &dataset,
        &calendar,
        Theme::dark(true),
        Units::Ml,
        AFTERNOON,
    )
    .join("\n");

    for alarming in [Tone::Error, Tone::Warning] {
        assert!(
            !painted.contains(&format!("\u{1b}[{}m", alarming.sgr())),
            "the day table paints something as {alarming:?}"
        );
    }
}

#[test]
fn sleep_start_accepts_an_explicit_time() {
    let parsed = Cli::try_parse_from(["h", "sleep", "start", "--start", "28m ago"]);
    assert!(parsed.is_ok(), "{parsed:?}");
}

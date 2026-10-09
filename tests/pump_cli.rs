//! Pumping is reachable through the same command and shell surfaces as other logs.
use app::cli::Cli;
use app::interactive::catalog;
use app::tui::{App, Intent, Motion};
use clap::Parser;

#[test]
fn pump_amounts_times_and_timer_actions_are_scriptable() {
    for arguments in [
        vec![
            "pump",
            "log",
            "--at",
            "32 mins ago",
            "--amount",
            "120",
            "--units",
            "ml",
            "--duration",
            "15",
            "--notes",
            "morning",
        ],
        vec![
            "pump", "log", "--left", "0", "--right", "2.5", "--units", "oz",
        ],
        vec!["pump", "log"],
        vec!["pump", "start", "--start", "20 mins ago"],
        vec!["pump", "pause", "--at", "5 mins ago"],
        vec!["pump", "resume", "--at", "3 mins ago"],
        vec![
            "pump", "stop", "--at", "now", "--amount", "90", "--units", "ml",
        ],
        vec!["pump", "cancel"],
        vec!["pump", "status"],
    ] {
        let parsed = Cli::try_parse_from(std::iter::once("h").chain(arguments.clone()));
        assert!(parsed.is_ok(), "{arguments:?}: {parsed:?}");
    }
}

#[test]
fn pump_total_cannot_be_mixed_with_side_amounts() {
    for arguments in [
        ["pump", "log", "--amount", "90", "--left", "30"],
        ["pump", "stop", "--amount", "90", "--right", "60"],
    ] {
        let error = Cli::try_parse_from(std::iter::once("h").chain(arguments)).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::ArgumentConflict);
    }
}

#[test]
fn fourth_home_item_reaches_pump_logging_without_running_on_a_digit() {
    let mut app = App::new();
    assert_eq!(app.apply(Motion::Highlight(3), 20), Intent::Stay);
    assert_eq!(app.rows()[app.cursor()].label, "Log pumping");
    assert_eq!(app.apply(Motion::Open, 20), Intent::Stay);
    let Intent::Run(path) = app.apply(Motion::Open, 20) else {
        panic!("pumping opens with a logging command");
    };
    assert_eq!(path.0, ["pump", "log"]);
    assert!(
        catalog::command_paths()
            .iter()
            .any(|path| path.0 == ["pump", "status"])
    );
}

//! The library surface, exercised from outside the crate.
//!
//! The binary's name is decided when the project is created, so these tests
//! drive the library rather than spawning `target/debug/<binary>`. Everything
//! `main.rs` calls is public and reachable here, which is the point of keeping
//! the binary thin.

use app::cli::{Cli, Command, ConfigAction};
use app::commands::config::show::lines;
use app::commands::greet::{greeting, resolve_name};
use app::commands::info::facts;
use app::config::{self, Config};
use app::prompt::{Question, Reply, interpret, unanswerable_message};
use app::theme::{Theme, Tone};
use clap::{CommandFactory, Parser};

#[test]
fn the_command_line_surface_is_well_formed() {
    Cli::command().debug_assert();
}

#[test]
fn every_action_is_reachable_without_a_prompt() {
    let greet = Cli::try_parse_from(["tool", "greet", "--name", "Ada"]).expect("greet parses");
    assert_eq!(
        greet.command,
        Command::Greet {
            name: Some("Ada".to_owned())
        }
    );

    let info = Cli::try_parse_from(["tool", "info"]).expect("info parses");
    assert_eq!(info.command, Command::Info);

    let show = Cli::try_parse_from(["tool", "config", "show"]).expect("config show parses");
    assert_eq!(
        show.command,
        Command::Config {
            action: ConfigAction::Show
        }
    );

    let set = Cli::try_parse_from(["tool", "config", "set", "verbose", "true", "--verbose"])
        .expect("config set parses");
    assert!(set.verbose);
    assert_eq!(
        set.command,
        Command::Config {
            action: ConfigAction::Set {
                key: Some("verbose".to_owned()),
                value: Some("true".to_owned()),
            }
        }
    );
}

#[test]
fn a_greeting_names_the_person() {
    assert_eq!(greeting("Hello", "Ada"), "Hello, Ada!");
}

#[test]
fn a_configured_name_stands_in_for_the_flag() {
    assert_eq!(resolve_name(None, "Grace"), Some("Grace".to_owned()));
    assert_eq!(resolve_name(None, ""), None);
}

#[test]
fn info_reports_the_running_version() {
    assert!(
        facts("/tmp/config.toml")
            .iter()
            .any(|(key, _)| *key == "version")
    );
}

#[test]
fn the_configuration_round_trips_through_its_file_format() {
    let mut config = Config::default();
    config.set("greeting", "Howdy").expect("a greeting is text");
    let text = config::serialize(&config).expect("serializing");
    assert_eq!(config::parse(&text).expect("parsing"), config);
}

#[test]
fn every_setting_is_shown_as_one_key_value_line() {
    assert_eq!(lines(&Config::default()).len(), Config::KEYS.len());
}

#[test]
fn an_omitted_value_is_asked_for_only_when_there_is_somebody_to_ask() {
    let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
    assert!(unanswerable_message(&question).contains("--name <NAME>"));
    assert_eq!(
        interpret(" Ada \n", question.choices, question.default),
        Reply::Accepted("Ada".to_owned())
    );
}

#[test]
fn piped_output_carries_no_escape_sequences() {
    let plain = Theme::dark(false);
    assert_eq!(plain.paint(Tone::Success, "done"), "done");
}

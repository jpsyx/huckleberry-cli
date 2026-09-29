use app::{
    cli::Cli,
    interactive::{
        self, EntryMode,
        session::{SessionOptions, load_context},
    },
    theme::Theme,
};
use clap::Parser;

#[test]
fn bare_cli_parses_without_command() {
    assert!(Cli::try_parse_from(["h"]).unwrap().command.is_none());
}

#[test]
fn global_only_cli_enters_menu_when_terminal() {
    let cli = Cli::try_parse_from(["h", "--verbose"]).unwrap();
    assert!(cli.verbose);
    assert_eq!(
        interactive::entry_mode(cli.command.is_some(), true, true),
        EntryMode::Interactive
    );
    assert_eq!(interactive::entry_mode(false, true, false), EntryMode::Help);
    assert_eq!(
        interactive::entry_mode(true, false, false),
        EntryMode::Command
    );
}

#[test]
fn nonterminal_bare_cli_prints_help_and_exits() {
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
        .arg("--config")
        .arg("/definitely/missing/config.toml")
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    assert!(output.stderr.is_empty());
}

#[test]
fn context_reload_uses_new_config_and_child() {
    let directory = std::env::temp_dir().join(format!("h-session-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("config.toml");
    std::fs::write(&path, "child = 'first'\nunits = 'ml'\n").unwrap();
    let mut options = SessionOptions {
        config: Some(path.clone()),
        ..Default::default()
    };
    let context = load_context(&options, Theme::dark(false)).unwrap();
    assert_eq!(context.config.child(), Some("first"));
    std::fs::write(&path, "child = 'second'\nunits = 'oz'\n").unwrap();
    options.child = Some("override".into());
    let context = load_context(&options, Theme::dark(false)).unwrap();
    assert_eq!(context.config.child(), Some("second"));
    assert_eq!(context.child_override.as_deref(), Some("override"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn typed_cancellation_returns_to_menu() {
    let error = anyhow::Error::new(app::prompt::Cancelled).context("while collecting notes");
    assert!(app::prompt::is_cancelled(&error));
    assert!(!app::prompt::is_cancelled(&anyhow::anyhow!(
        "network failure"
    )));
}

//! Version flags bypass setup; update checks remain optional and scriptable.
use app::cli::Cli;
use clap::{CommandFactory, Parser, error::ErrorKind};

#[test]
fn every_version_alias_prints_the_same_version() {
    let expected = Cli::try_parse_from(["h", "--version"])
        .unwrap_err()
        .to_string();
    for flag in ["-v", "-V", "--version"] {
        let result = Cli::try_parse_from(["h", flag]).unwrap_err();
        assert_eq!(result.kind(), ErrorKind::DisplayVersion);
        assert_eq!(result.to_string(), expected);
    }
}

#[test]
fn version_aliases_remain_available_under_subcommands() {
    for arguments in [
        vec!["h", "info", "--version"],
        vec!["h", "config", "show", "-V"],
        vec!["h", "sleep", "start", "-v"],
    ] {
        let result = Cli::try_parse_from(arguments).unwrap_err();
        assert_eq!(result.kind(), ErrorKind::DisplayVersion);
        assert!(result.to_string().contains(env!("CARGO_PKG_VERSION")));
    }
}

#[test]
fn diagnostics_keep_the_long_flag() {
    assert!(
        Cli::try_parse_from(["h", "--verbose", "info"])
            .unwrap()
            .verbose
    );
}

#[test]
fn info_accepts_an_explicit_update_check() {
    let matches = Cli::command()
        .try_get_matches_from(["h", "info", "--check-update"])
        .unwrap();
    assert!(
        matches
            .subcommand_matches("info")
            .unwrap()
            .get_flag("check_update")
    );
}

fn invoke(arguments: &[&str]) -> std::process::Output {
    std::process::Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
        .env_clear()
        .args(arguments)
        .output()
        .unwrap()
}

#[test]
fn short_version_never_loads_an_unusable_config_path() {
    let output = invoke(&["--config", "/dev/null/not-a-file", "-v"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("huckleberry-cli {}\n", env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn optional_offline_info_check_appends_facts_without_setup_or_a_snapshot_read() {
    let path =
        std::env::temp_dir().join(format!("h-version-test-{}-absent.toml", std::process::id()));
    let config = path.to_str().unwrap();
    let plain = invoke(&["--config", config, "info"]);
    let checked = invoke(&[
        "--config",
        config,
        "--offline",
        "absent-snapshot.json",
        "info",
        "--check-update",
    ]);
    assert!(plain.status.success());
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let plain = String::from_utf8(plain.stdout).unwrap();
    let checked = String::from_utf8(checked.stdout).unwrap();
    assert_eq!(
        checked,
        format!("{plain}latest_version=\nupdate_status=offline\n")
    );
    assert!(!plain.contains("update_status="));
}

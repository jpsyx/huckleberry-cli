//! Resolve event time before collecting or validating activity details.
use std::process::{Command, Stdio};

/// A configuration file with first-use setup already answered, so these tests
/// exercise the ordering they are about rather than the setup gate.
fn configured(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir()
        .join(format!("huckleberry-{name}-{}", std::process::id()))
        .join("config.toml");
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("a directory");
    std::fs::write(&path, "day_mode = \"continuous\"\n").expect("a configuration");
    path
}

#[test]
fn manual_sleep_rejects_reversed_relative_times_before_connecting() {
    let config = configured("manual-order");
    let output = Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
        .arg("--config")
        .arg(config)
        .args(["--child", "test-child", "sleep", "manual"])
        .args(["--start", "32 min ago", "--end", "40 min ago"])
        .env("HUCKLEBERRY_EMAIL", "test@example.invalid")
        .env("HUCKLEBERRY_PASSWORD", "test-password")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(!output.status.success());
    assert!(error.contains("end must be after its start"), "{error}");
}

#[test]
fn invalid_time_is_reported_before_missing_or_invalid_activity_details() {
    let config = configured("logging-order");
    for arguments in [
        vec!["diaper"],
        vec!["potty"],
        vec!["growth"],
        vec!["feed", "bottle", "--amount", "0"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_huckleberry-cli"))
            .arg("--config")
            .arg(&config)
            .args(["--child", "test-child"])
            .args(&arguments)
            .args(["--at", "invalid-time"])
            .env("HUCKLEBERRY_EMAIL", "test@example.invalid")
            .env("HUCKLEBERRY_PASSWORD", "test-password")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success(), "{arguments:?}");
        assert!(
            error.contains("`invalid-time` is not a time"),
            "{arguments:?} asked for activity details before resolving time: {error}"
        );
    }
}

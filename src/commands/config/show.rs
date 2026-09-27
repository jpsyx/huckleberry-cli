//! The `config show` command: the effective configuration, as data.
//!
//! Effective means what the tool will actually use: the file's values where it
//! has them and the defaults everywhere else, so the output never depends on
//! whether the file exists yet. One `key=value` line per setting, on stdout,
//! so `config show | grep` is a reasonable thing to type.

use anyhow::Result;

use crate::config::Config;

/// One `key=value` line per setting, in a stable order. Pure, so the shape of
/// the output is testable.
#[must_use]
pub fn lines(config: &Config) -> Vec<String> {
    config
        .entries()
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Prints the effective configuration on stdout.
pub fn run(config: &Config) -> Result<()> {
    for line in lines(config) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_gets_exactly_one_line() {
        let lines = lines(&Config::default());
        assert_eq!(lines.len(), Config::KEYS.len());
        for key in Config::KEYS {
            assert!(
                lines
                    .iter()
                    .any(|line| line.starts_with(&format!("{key}="))),
                "{key} is not shown"
            );
        }
    }

    #[test]
    fn a_line_carries_the_value_unquoted() {
        let mut config = Config::default();
        config.set("greeting", "Good day").expect("a greeting");
        assert!(lines(&config).contains(&"greeting=Good day".to_owned()));
    }
}

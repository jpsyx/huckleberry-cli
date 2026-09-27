//! The `info` command: what this build is, as data.
//!
//! Stdout is the machine channel, so the output is plain `key=value` lines: no
//! color, no table drawing, nothing a caller has to strip before parsing. The
//! configuration path is one of the facts, because "where does this tool keep
//! its settings" is a question people ask constantly.

use std::path::Path;

use anyhow::Result;

use crate::APP_NAME;

/// The facts, in a stable order. Pure, so the shape of the output is testable.
#[must_use]
pub fn facts(config_path: &str) -> Vec<(&'static str, String)> {
    vec![
        ("name", APP_NAME.to_owned()),
        ("package", env!("CARGO_PKG_NAME").to_owned()),
        ("version", env!("CARGO_PKG_VERSION").to_owned()),
        ("config", config_path.to_owned()),
    ]
}

/// Prints one `key=value` line per fact.
pub fn run(config_path: &Path) -> Result<()> {
    for (key, value) in facts(&config_path.display().to_string()) {
        println!("{key}={value}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_facts_lead_with_the_name_and_carry_a_version() {
        let facts = facts("/tmp/config.toml");
        assert_eq!(facts.first().map(|(key, _)| *key), Some("name"));
        assert!(
            facts
                .iter()
                .any(|(key, value)| *key == "version" && !value.is_empty())
        );
    }

    #[test]
    fn the_configuration_file_is_one_of_the_facts() {
        let facts = facts("/tmp/config.toml");
        assert!(
            facts
                .iter()
                .any(|(key, value)| *key == "config" && value == "/tmp/config.toml")
        );
    }

    #[test]
    fn no_fact_is_blank() {
        for (key, value) in facts("/tmp/config.toml") {
            assert!(
                !key.is_empty() && !value.is_empty(),
                "{key} has nothing to say"
            );
        }
    }
}

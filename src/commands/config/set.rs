//! The `config set` command: change one setting and save it.
//!
//! `config set greeting Howdy` is the whole thing non-interactively. Leave the
//! key out on a terminal and it offers the settings by number; leave the value
//! out and it asks for one, offering the current value as the default. With no
//! terminal, both questions become a failure naming the argument instead.

use std::path::Path;

use anyhow::{Context, Result, anyhow};

use crate::commands::{detail, narrate};
use crate::config::{self, Config};
use crate::prompt::{self, Choice, Question};
use crate::theme::Theme;

/// The settings offered when no key was given, each with the line that says
/// what it does. Pure, so the list cannot drift from the settings themselves.
#[must_use]
pub fn choices() -> Vec<Choice<'static>> {
    Config::KEYS
        .iter()
        .map(|key| Choice {
            value: key,
            hint: Config::describe(key).unwrap_or_default(),
        })
        .collect()
}

/// Sets one setting and writes the file.
pub fn run(
    key: Option<&str>,
    value: Option<&str>,
    path: &Path,
    config: &Config,
    theme: Theme,
    verbose: bool,
) -> Result<()> {
    let key = key.map_or_else(|| ask_which_setting(theme), |given| Ok(given.to_owned()))?;
    let current = config
        .get(&key)
        .ok_or_else(|| anyhow!(config::unknown_key_message(&key)))?;
    let value = value.map_or_else(
        || ask_for_value(&key, &current, theme),
        |given| Ok(given.to_owned()),
    )?;

    let mut updated = config.clone();
    updated
        .set(&key, &value)
        .with_context(|| format!("setting {key}"))?;

    narrate(theme, &format!("Saving {key} to {}.", path.display()));
    detail(
        verbose,
        theme,
        "The file is rewritten from the settings, so hand-written comments in it are lost.",
    );
    config::save(path, &updated)?;

    let stored = updated.get(&key).unwrap_or_default();
    eprintln!("{}", theme.success(&format!("set {key}={stored}")));
    Ok(())
}

/// Offers the settings and returns the one that was picked.
fn ask_which_setting(theme: Theme) -> Result<String> {
    let choices = choices();
    let question =
        Question::new("setting to change", "Which setting?", "<KEY>").with_choices(&choices);
    prompt::ask(&question, theme)
}

/// Asks for the new value, offering the current one as the default.
fn ask_for_value(key: &str, current: &str, theme: Theme) -> Result<String> {
    let label = format!("New value for {key}?");
    let question = Question::new("value to set", &label, "<VALUE>");
    let question = if current.is_empty() {
        question
    } else {
        question.with_default(current)
    };
    prompt::ask(&question, theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_offered_with_a_hint() {
        let offered = choices();
        assert_eq!(offered.len(), Config::KEYS.len());
        for (choice, key) in offered.iter().zip(Config::KEYS) {
            assert_eq!(choice.value, key);
            assert!(!choice.hint.is_empty(), "{key} is offered without a hint");
        }
    }
}

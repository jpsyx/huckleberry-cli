//! The `greet` command: both audiences in one place.
//!
//! The name is resolved in three steps, which is the pattern every command
//! that needs a value should copy: the flag, then the configuration, then a
//! question (or, with no terminal, a failure naming the flag). The greeting
//! itself is a pure function, which is the part worth testing.

use anyhow::Result;

use crate::config::Config;
use crate::prompt::{self, Question};
use crate::theme::Theme;

/// Builds the greeting. A blank name greets the world, and a blank opening
/// word falls back to `Hello`, so a hand-edited configuration cannot produce
/// a sentence with a hole in it.
#[must_use]
pub fn greeting(opening: &str, name: &str) -> String {
    let opening = opening.trim();
    let opening = if opening.is_empty() { "Hello" } else { opening };
    let name = name.trim();
    let name = if name.is_empty() { "world" } else { name };
    format!("{opening}, {name}!")
}

/// Who to greet, from the flag or the configuration. `None` means nobody has
/// said, so the command has to ask.
#[must_use]
pub fn resolve_name(flag: Option<&str>, configured: &str) -> Option<String> {
    flag.map(ToOwned::to_owned).or_else(|| {
        let configured = configured.trim();
        (!configured.is_empty()).then(|| configured.to_owned())
    })
}

/// Prints the greeting on stdout, asking who to greet when nothing has said
/// and there is a terminal to ask on.
pub fn run(name: Option<&str>, config: &Config, theme: Theme) -> Result<()> {
    let name = resolve_name(name, &config.name).map_or_else(|| ask_for_name(theme), Ok)?;
    println!("{}", greeting(&config.greeting, &name));
    Ok(())
}

/// Asks who to greet. Without a terminal there is nobody to ask, so the
/// failure names the flag that answers the question instead.
fn ask_for_name(theme: Theme) -> Result<String> {
    let question = Question::new("name to greet", "Who should I greet?", "--name <NAME>");
    prompt::ask(&question, theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_greeting_names_the_person() {
        assert_eq!(greeting("Hello", "Ada"), "Hello, Ada!");
    }

    #[test]
    fn the_configured_opening_word_is_used() {
        assert_eq!(greeting("Howdy", "Ada"), "Howdy, Ada!");
    }

    #[test]
    fn surrounding_space_is_not_part_of_the_name() {
        assert_eq!(greeting("Hello", "  Ada \n"), "Hello, Ada!");
    }

    #[test]
    fn an_empty_name_greets_the_world() {
        assert_eq!(greeting("Hello", "   "), "Hello, world!");
    }

    #[test]
    fn an_empty_opening_word_still_greets() {
        assert_eq!(greeting("  ", "Ada"), "Hello, Ada!");
    }

    #[test]
    fn the_flag_wins_over_the_configuration() {
        assert_eq!(resolve_name(Some("Ada"), "Grace"), Some("Ada".to_owned()));
    }

    #[test]
    fn the_configuration_answers_when_the_flag_is_absent() {
        assert_eq!(resolve_name(None, "Grace"), Some("Grace".to_owned()));
    }

    #[test]
    fn with_neither_there_is_nobody_to_greet_yet() {
        assert_eq!(resolve_name(None, "   "), None);
    }
}

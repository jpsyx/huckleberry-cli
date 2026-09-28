//! `config`: reading and changing the stored settings.

use anyhow::Result;

use crate::cli::ConfigAction;
use crate::config::Config;
use crate::prompt::{self, Choice, Question};
use crate::session::Context;

/// Runs the chosen action.
pub fn run(context: &Context, action: &ConfigAction) -> Result<()> {
    match action {
        ConfigAction::Show => {
            context.present(
                "⚙️ Settings",
                &crate::render::output::settings_fields(&context.config),
                &lines(&context.config),
            );
            if context.verbose {
                for (name, path) in
                    crate::session::describe_paths(&context.config_path, &context.credentials_path)
                {
                    context.detail(&format!("{name}: {path}"));
                }
            }
            Ok(())
        }
        ConfigAction::Set { key, value } => set(context, key.clone(), value.clone()),
        ConfigAction::Path => {
            let paths =
                crate::session::describe_paths(&context.config_path, &context.credentials_path);
            let machine: Vec<String> = paths
                .iter()
                .map(|(name, path)| format!("{name}\t{path}"))
                .collect();
            context.present(
                "📁 Settings files",
                &[
                    ("Settings", paths[0].1.clone()),
                    ("Credentials", paths[1].1.clone()),
                ],
                &machine,
            );
            Ok(())
        }
    }
}

/// The effective settings, one `key=value` line each.
#[must_use]
pub fn lines(config: &Config) -> Vec<String> {
    config
        .entries()
        .into_iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect()
}

/// Changes one setting, asking for whatever was not given.
fn set(context: &Context, key: Option<String>, value: Option<String>) -> Result<()> {
    let key = match key {
        Some(given) => given,
        None => ask_for_key(context)?,
    };
    let value = match value {
        Some(given) => given,
        None => ask_for_value(context, &key)?,
    };

    let mut config = context.config.clone();
    config.set(&key, &value)?;
    crate::config::save(&context.config_path, &config)?;

    let fields = crate::render::output::settings_fields(&config);
    let changed: Vec<_> = Config::KEYS
        .iter()
        .zip(fields)
        .filter(|(name, _)| **name == key)
        .map(|(_, field)| field)
        .collect();
    context.receipt("⚙️ Setting saved", &changed, &[]);
    context.detail(&format!(
        "saved to {} (any comments in it are rewritten away)",
        context.config_path.display()
    ));
    Ok(())
}

fn ask_for_key(context: &Context) -> Result<String> {
    let choices: Vec<Choice<'_>> = Config::KEYS
        .iter()
        .map(|key| Choice {
            value: key,
            hint: Config::describe(key).unwrap_or(""),
        })
        .collect();
    let question =
        Question::new("setting to change", "Which setting?", "<KEY>").with_choices(&choices);
    prompt::ask(&question, context.theme)
}

fn ask_for_value(context: &Context, key: &str) -> Result<String> {
    let current = context.config.get(key).unwrap_or_default();
    let label = format!("{}?", Config::describe(key).unwrap_or("the new value"));
    let mut question = Question::new("value", &label, "<VALUE>");
    if !current.is_empty() {
        question = question.with_default(&current);
    }
    prompt::ask(&question, context.theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_shown_as_one_key_value_line() {
        let shown = lines(&Config::default());
        assert_eq!(shown.len(), Config::KEYS.len());
        for line in &shown {
            assert!(line.contains('='), "{line}");
        }
    }

    #[test]
    fn the_lines_are_in_the_order_the_keys_are_declared() {
        let shown = lines(&Config::default());
        for (line, key) in shown.iter().zip(Config::KEYS) {
            assert!(line.starts_with(&format!("{key}=")), "{line}");
        }
    }
}

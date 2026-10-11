//! `config`: reading and changing the stored settings.

use anyhow::Result;

use crate::cli::ConfigAction;
use crate::config::Config;
use crate::prompt::Choice;
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
    let model =
        super::utility_form::SettingsForm::create_settings(context.config.clone(), key, value)?;
    let answer = crate::prompt::flow::runner::read(
        model,
        huckleberry_api::client::now_seconds(),
        context.theme,
    )?;
    execute(context, answer)
}

pub(crate) fn execute(
    context: &Context,
    answer: super::utility_form::SettingsSubmission,
) -> Result<()> {
    let super::utility_form::SettingsSubmission { key, value } = answer;
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

/// Finite setting values use the same lists as recording prompts.
#[must_use]
pub fn value_choices(key: &str) -> Vec<Choice<'static>> {
    let values: &[&str] = match key {
        "units" => &["ml", "oz"],
        "measurements" => &["metric", "imperial"],
        "verbose" => &["true", "false"],
        _ => &[],
    };
    values
        .iter()
        .map(|value| Choice { value, hint: "" })
        .collect()
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

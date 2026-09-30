//! Editors for every clap argument, using typed metadata and specialized forms.
mod edit;
pub mod fields;
mod value;
use super::{
    catalog::{CommandPath, metadata},
    draft::CommandDraft,
};
use crate::{
    prompt::select::{self, MenuItem},
    theme::Theme,
};
use anyhow::Result;
use clap::ArgAction;

/// Which input widget implements an argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionControl {
    /// A fixed set of words.
    Choice,
    /// On or off.
    Boolean,
    /// Free text or number.
    Text,
    /// Masked input.
    Secret,
    /// Multiple values.
    Repeated,
    /// An entry-specific edit form.
    Form,
    /// Global overrides live in Session options.
    Session,
    /// Read help without executing.
    Help,
    /// Read the build version.
    Version,
}
/// An actual argument editor binding.
pub struct OptionBinding {
    /// clap's stable argument ID.
    pub argument_id: String,
    /// Widget used by the option editor.
    pub control: OptionControl,
}

/// Derives the editor inventory from the actual command surface.
#[must_use]
pub fn bindings(path: &CommandPath) -> Vec<OptionBinding> {
    metadata(path).map_or_else(Vec::new, |command| {
        command
            .get_arguments()
            .map(|arg| {
                let id = arg.get_id().as_str();
                let control = match id {
                    "help" => OptionControl::Help,
                    "version" => OptionControl::Version,
                    "config" | "child" | "offline" | "verbose" => OptionControl::Session,
                    "password" => OptionControl::Secret,
                    "set" => OptionControl::Form,
                    _ if !arg.get_possible_values().is_empty() => OptionControl::Choice,
                    _ if matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse) => {
                        OptionControl::Boolean
                    }
                    _ if matches!(arg.get_action(), ArgAction::Append) => OptionControl::Repeated,
                    _ => OptionControl::Text,
                };
                OptionBinding {
                    argument_id: id.into(),
                    control,
                }
            })
            .collect()
    })
}

/// Updates an override while removing mutually exclusive choices.
pub fn set_value(draft: &mut CommandDraft, id: &str, values: Vec<String>) {
    if let Some(command) = metadata(&draft.path)
        && let Some(arg) = command
            .get_arguments()
            .find(|arg| arg.get_id().as_str() == id)
    {
        for conflict in command.get_arg_conflicts_with(arg) {
            draft.clear(conflict.get_id().as_str());
        }
    }
    draft.set(id, values);
}

/// Edits command-specific overrides, retaining them until Run or Back.
pub async fn edit(
    theme: Theme,
    globals: &mut super::session::SessionOptions,
    draft: &mut CommandDraft,
) -> Result<()> {
    loop {
        let bindings = bindings(&draft.path);
        let mut labels = vec!["Done".to_owned()];
        labels.extend(bindings.iter().map(|binding| label(draft, binding)));
        let items = labels
            .into_iter()
            .map(|label| MenuItem {
                label,
                detail: None,
            })
            .collect::<Vec<_>>();
        let index = select::choose("Command options", &items, 0, theme)?;
        if index == 0 {
            return Ok(());
        }
        let binding = &bindings[index - 1];
        Box::pin(apply_binding(theme, globals, draft, binding)).await?;
    }
}

fn label(draft: &CommandDraft, binding: &OptionBinding) -> String {
    let value = draft.values.get(&binding.argument_id).map_or_else(
        || "default / ask".into(),
        |values| {
            if binding.control == OptionControl::Secret {
                "(hidden)".into()
            } else {
                values.join(", ")
            }
        },
    );
    format!(
        "{}: {value}",
        crate::render::output::words(&binding.argument_id)
    )
}

/// Finite option values, including configuration keys whose clap type is text.
#[must_use]
pub fn choices_for(draft: &CommandDraft, id: &str) -> Vec<String> {
    if draft.path.0 == ["config", "set"] {
        if id == "key" {
            return crate::config::Config::KEYS
                .iter()
                .map(|key| (*key).into())
                .collect();
        }
        if id == "value" {
            let key = draft.values.get("key").and_then(|values| values.first());
            return key.map_or_else(Vec::new, |key| {
                crate::commands::settings::value_choices(key)
                    .iter()
                    .map(|choice| choice.value.into())
                    .collect()
            });
        }
    }
    metadata(&draft.path)
        .and_then(|command| {
            command
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .map(|arg| {
                    arg.get_possible_values()
                        .iter()
                        .map(|value| value.get_name().into())
                        .collect()
                })
        })
        .unwrap_or_default()
}

async fn apply_binding(
    theme: Theme,
    globals: &mut super::session::SessionOptions,
    draft: &mut CommandDraft,
    binding: &OptionBinding,
) -> Result<()> {
    match binding.control {
        OptionControl::Form => {
            edit::collect(&super::session::load_context(globals, theme)?, draft).await?;
        }
        OptionControl::Session => {
            let previous = globals.clone();
            *globals = super::session::edit_options(globals, theme).await?;
            if draft.invalidate_context(&previous, globals) {
                eprintln!("Context changed. Pending options cleared; select the entry again.");
                super::pause(theme)?;
            }
        }
        OptionControl::Help => {
            if let Some(mut command) = metadata(&draft.path) {
                eprintln!("{}", command.render_long_help());
                super::pause(theme)?;
            }
        }
        OptionControl::Version => {
            eprintln!("{}", env!("CARGO_PKG_VERSION"));
            super::pause(theme)?;
        }
        _ => {
            value::edit(
                &super::session::load_context(globals, theme)?,
                draft,
                binding,
            )
            .await?;
        }
    }
    Ok(())
}

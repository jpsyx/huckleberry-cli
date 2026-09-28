//! Pending options become typed clap commands without a shell.
use super::{
    catalog::{CommandPath, metadata},
    session::SessionOptions,
};
use crate::cli::Cli;
use anyhow::{Result, anyhow};
use clap::{ArgAction, Parser};
use std::{collections::BTreeMap, ffi::OsString};

/// An unsubmitted command and its explicit option overrides.
#[derive(Debug, Clone)]
pub struct CommandDraft {
    /// Canonical command words.
    pub path: CommandPath,
    /// Values indexed by clap argument IDs.
    pub values: BTreeMap<String, Vec<String>>,
}
impl CommandDraft {
    /// Starts with ordinary command defaults and prompts.
    #[must_use]
    pub const fn new(path: CommandPath) -> Self {
        Self {
            path,
            values: BTreeMap::new(),
        }
    }
    /// Sets an argument, preserving repeated values as separate words.
    pub fn set(&mut self, id: &str, values: Vec<String>) {
        self.values.insert(id.into(), values);
    }
    /// Returns an argument to its ordinary default or prompt.
    pub fn clear(&mut self, id: &str) {
        self.values.remove(id);
    }
    /// Validates the pending command with the actual CLI parser.
    pub fn resolve(&self, globals: &SessionOptions) -> Result<Cli> {
        let meta = metadata(&self.path).ok_or_else(|| anyhow!("unknown command"))?;
        let mut words = vec![OsString::from("hb")];
        words.extend(self.path.0.iter().map(OsString::from));
        let mut positionals = Vec::new();
        for (id, values) in &self.values {
            let arg = meta
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .ok_or_else(|| anyhow!("unknown option {id}"))?;
            if let Some(index) = arg.get_index() {
                positionals.push((index, values));
            } else {
                append_option(&mut words, arg, values);
            }
        }
        positionals.sort_by_key(|(index, _)| *index);
        for (_, values) in positionals {
            words.extend(values.iter().map(OsString::from));
        }
        // Keep secrets out of clap's diagnostic excerpts.
        let mut cli = Cli::try_parse_from(words).map_err(|error| {
            anyhow!(
                "Invalid options ({:?}); review the selected values.",
                error.kind()
            )
        })?;
        cli.config.clone_from(&globals.config);
        cli.child.clone_from(&globals.child);
        cli.offline.clone_from(&globals.offline);
        cli.verbose = globals.verbose;
        Ok(cli)
    }
}

fn append_option(words: &mut Vec<OsString>, arg: &clap::Arg, values: &[String]) {
    let flag = arg
        .get_long()
        .map(|name| format!("--{name}"))
        .or_else(|| arg.get_short().map(|name| format!("-{name}")));
    let Some(flag) = flag else {
        return;
    };
    if matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse) {
        if values.first().is_some_and(|value| value == "true") {
            words.push(flag.into());
        }
    } else {
        for value in values {
            words.push(flag.clone().into());
            words.push(value.into());
        }
    }
}

/// Safe pending-option display, masking passwords.
#[must_use]
pub fn preview(draft: &CommandDraft) -> Vec<(String, String)> {
    draft
        .values
        .iter()
        .map(|(key, values)| {
            (
                key.clone(),
                if key == "password" {
                    "(hidden)".into()
                } else {
                    values.join(", ")
                },
            )
        })
        .collect()
}

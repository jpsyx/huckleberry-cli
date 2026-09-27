//! The command-line surface.
//!
//! Every action has a flag or a subcommand, so an agent can drive the tool
//! without ever meeting a prompt. Values a person may omit are `Option`s: the
//! command asks for them when there is a terminal to ask on, and fails with
//! the flag that would have answered when there is not. See
//! `docs/rules/cli-ux.md`.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// huckleberry-cli: a command-line tool.
#[derive(Debug, Clone, Parser)]
#[command(version, propagate_version = true, arg_required_else_help = true)]
pub struct Cli {
    /// Print detailed diagnostics to stderr.
    #[arg(short, long, global = true, env = "CLI_VERBOSE")]
    pub verbose: bool,

    /// Read and write this configuration file instead of the default one.
    #[arg(long, global = true, env = "CLI_CONFIG", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// The command to run.
    #[command(subcommand)]
    pub command: Command,
}

/// The things this tool can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Print a greeting.
    Greet {
        /// Who to greet. Left out on a terminal, you are asked for it.
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Print what this build is, one `key=value` line per fact.
    Info,
    /// Read and change the stored configuration.
    Config {
        /// What to do with the configuration.
        #[command(subcommand)]
        action: ConfigAction,
    },
}

/// What `config` can be asked to do.
#[derive(Debug, Clone, PartialEq, Eq, Subcommand)]
pub enum ConfigAction {
    /// Print the effective configuration, one `key=value` line per setting.
    Show,
    /// Set one setting and save it.
    Set {
        /// Which setting to change. Left out on a terminal, you are asked.
        key: Option<String>,

        /// The new value. Left out on a terminal, you are asked.
        value: Option<String>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn the_surface_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn a_bare_invocation_shows_the_help() {
        let error = Cli::try_parse_from(["tool"]).expect_err("a subcommand is required");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn verbose_is_accepted_after_a_subcommand() {
        let cli = Cli::try_parse_from(["tool", "info", "--verbose"]).expect("global flag parses");
        assert!(cli.verbose);
        assert_eq!(cli.command, Command::Info);
    }

    #[test]
    fn the_config_file_can_be_named_after_a_subcommand() {
        let cli = Cli::try_parse_from(["tool", "config", "show", "--config", "/tmp/a.toml"])
            .expect("global flag parses");
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/a.toml"))
        );
    }

    #[test]
    fn greet_takes_a_name_non_interactively() {
        let cli = Cli::try_parse_from(["tool", "greet", "--name", "Ada"]).expect("a name parses");
        assert_eq!(
            cli.command,
            Command::Greet {
                name: Some("Ada".to_owned())
            }
        );
    }

    #[test]
    fn greet_without_a_name_still_parses() {
        let cli = Cli::try_parse_from(["tool", "greet"]).expect("an omitted name parses");
        assert_eq!(cli.command, Command::Greet { name: None });
    }

    #[test]
    fn config_set_takes_both_values_non_interactively() {
        let cli = Cli::try_parse_from(["tool", "config", "set", "greeting", "Hi"])
            .expect("a key and a value parse");
        assert_eq!(
            cli.command,
            Command::Config {
                action: ConfigAction::Set {
                    key: Some("greeting".to_owned()),
                    value: Some("Hi".to_owned()),
                }
            }
        );
    }

    #[test]
    fn config_set_without_arguments_still_parses() {
        let cli = Cli::try_parse_from(["tool", "config", "set"]).expect("omitted values parse");
        assert_eq!(
            cli.command,
            Command::Config {
                action: ConfigAction::Set {
                    key: None,
                    value: None,
                }
            }
        );
    }

    #[test]
    fn an_unknown_subcommand_is_rejected() {
        assert!(Cli::try_parse_from(["tool", "frobnicate"]).is_err());
    }
}

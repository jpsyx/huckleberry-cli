//! Getting from a configuration file to a working client.
//!
//! Every networked command needs the same four things settled first: which
//! configuration file, which credentials, which child, and whether this run is
//! reading Huckleberry at all or a snapshot on disk. [`Context`] settles them
//! once, and hands each command something it can use.
//!
//! The one rule worth stating: **a command never signs in silently on a pipe.**
//! With a terminal, a missing password is a question. Without one, it is a
//! failure that names the flag and the environment variable that would have
//! answered, which is what stops an agent or a CI job hanging on a prompt it
//! cannot see.

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use huckleberry_api::{Credentials, Huckleberry};

use crate::cli::Cli;
use crate::config::Config;
use crate::credentials::{self, EMAIL_ENV, PASSWORD_ENV, Resolved};
use crate::domain::Calendar;
use crate::prompt::{self, Question};
use crate::theme::Theme;

/// Everything a command needs before it does anything.
pub struct Context {
    /// The effective settings.
    pub config: Config,
    /// Where they came from.
    pub config_path: PathBuf,
    /// Where the secrets live, beside them.
    pub credentials_path: PathBuf,
    /// The palette.
    pub theme: Theme,
    /// Whether to print detail.
    pub verbose: bool,
    /// The child `--child` named, if it named one.
    pub child_override: Option<String>,
    /// A snapshot to read instead of Huckleberry.
    pub offline: Option<PathBuf>,
}

impl Context {
    /// Reads the configuration the arguments point at.
    pub fn open(cli: &Cli, theme: Theme) -> Result<Self> {
        let config_path = cli
            .config
            .clone()
            .map_or_else(crate::config::default_path, Ok)?;
        let config = crate::config::load(&config_path)?;
        let verbose = cli.verbose || config.verbose;
        Ok(Self {
            credentials_path: credentials::path_beside(&config_path),
            config_path,
            config,
            theme,
            verbose,
            child_override: cli.child.clone(),
            offline: cli.offline.clone(),
        })
    }

    /// The calendar the family's days are counted in.
    pub fn calendar(&self) -> Result<Calendar> {
        self.config.calendar()
    }

    /// The window of days a command should read: the flag, then the setting.
    #[must_use]
    pub fn days(&self, flag: Option<u32>) -> u32 {
        flag.unwrap_or(self.config.days).clamp(1, 400)
    }

    /// What is on disk and in the environment, combined.
    pub fn credentials(&self) -> Result<Resolved> {
        let stored = credentials::load(&self.credentials_path)?;
        Ok(credentials::resolve(
            &stored,
            std::env::var(EMAIL_ENV).ok().as_deref(),
            std::env::var(PASSWORD_ENV).ok().as_deref(),
        ))
    }

    /// Writes the credentials back, keeping whatever the environment supplied
    /// out of the file.
    pub fn save_credentials(&self, resolved: &Resolved) -> Result<()> {
        credentials::save(&self.credentials_path, &credentials::to_store(resolved))
    }

    /// A client, signing in if it has to.
    ///
    /// # Errors
    ///
    /// When there is neither a saved session nor credentials, and no terminal
    /// to ask on.
    pub fn client(&self) -> Result<Huckleberry> {
        let resolved = self.credentials()?;
        self.client_from(&resolved)
    }

    /// A client built from credentials already in hand.
    pub fn client_from(&self, resolved: &Resolved) -> Result<Huckleberry> {
        if !resolved.is_usable() {
            bail!(not_signed_in_message());
        }
        let timezone = &self.config.timezone;
        let client = if let Some(session) = resolved.session.clone() {
            Huckleberry::resume(session, timezone)?
        } else {
            let (email, password) = resolved
                .email
                .clone()
                .zip(resolved.password.clone())
                .ok_or_else(|| anyhow::anyhow!(not_signed_in_message()))?;
            Huckleberry::new(Credentials::new(&email, &password), timezone)?
        };
        // A resumed client is given the password too, so a refresh token that
        // has finally lapsed becomes a fresh sign-in rather than a failure.
        Ok(match (&resolved.email, &resolved.password) {
            (Some(email), Some(password)) => {
                client.with_credentials(Credentials::new(email, password))
            }
            _ => client,
        })
    }

    /// Asks for an email address and a password, when there is somebody to
    /// ask. Used by `auth login`, which is the one command whose whole job
    /// this is.
    pub fn ask_for_credentials(
        &self,
        email: Option<String>,
        password: Option<String>,
    ) -> Result<(String, String)> {
        let existing = self.credentials()?;
        let email = if let Some(known) = email.or(existing.email) {
            known
        } else {
            prompt::ask(
                &Question::new(
                    "email address",
                    "Which email is the Huckleberry account under?",
                    "--email <ADDRESS>",
                ),
                self.theme,
            )?
        };
        let password = match password.or(existing.password) {
            Some(known) => known,
            None => prompt::ask_secret(
                &Question::new(
                    "password",
                    "Password:",
                    &format!("--password <PASSWORD> or {PASSWORD_ENV}"),
                ),
                self.theme,
            )?,
        };
        Ok((email, password))
    }

    /// Says what the tool is about to do, on stderr, always.
    pub fn narrate(&self, message: &str) {
        eprintln!("{}", self.theme.info(message));
    }

    /// Says the same thing in more detail, only under `--verbose`.
    pub fn detail(&self, message: &str) {
        if self.verbose {
            eprintln!("{}", self.theme.muted(message));
        }
    }

    /// Reports that something worked.
    pub fn report(&self, message: &str) {
        eprintln!("{}", self.theme.success(message));
    }

    /// Reports something that worked but deserves a second look.
    pub fn warn(&self, message: &str) {
        eprintln!("{}", self.theme.warning(message));
    }

    /// Points at something about the record that is worth a second look.
    ///
    /// Yellow, and never red: nothing has gone wrong with the tool, which is
    /// what `warn` is for. See `docs/rules/cli-ux.md`.
    pub fn attention(&self, message: &str) {
        eprintln!("{}", self.theme.attention(message));
    }
}

/// The failure when nobody has signed in and there is nobody to ask.
///
/// Names the command as it was actually invoked, because `install.sh --name`
/// means this tool does not know what it is called.
#[must_use]
pub fn not_signed_in_message() -> String {
    sign_in_hint(&crate::program_name())
}

/// The same message, for a given command name.
#[must_use]
pub fn sign_in_hint(command: &str) -> String {
    format!("not signed in: run `{command} auth login`, or set {EMAIL_ENV} and {PASSWORD_ENV}")
}

/// Which child to act on, given what the flag and the configuration say.
///
/// `None` means nobody has said, which is the command's cue to offer the
/// children on the account.
#[must_use]
pub fn resolve_child(flag: Option<&str>, configured: Option<&str>) -> Option<String> {
    flag.map(str::trim)
        .filter(|cid| !cid.is_empty())
        .or(configured)
        .map(ToOwned::to_owned)
}

/// Where the two files live, as `config show --verbose` and `config path`
/// print them.
#[must_use]
pub fn describe_paths(config_path: &Path, credentials_path: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("config", config_path.display().to_string()),
        ("credentials", credentials_path.display().to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_wins_over_the_setting() {
        assert_eq!(
            resolve_child(Some("from-flag"), Some("from-config")),
            Some("from-flag".to_owned())
        );
    }

    #[test]
    fn the_setting_stands_in_when_the_flag_is_absent() {
        assert_eq!(
            resolve_child(None, Some("from-config")),
            Some("from-config".to_owned())
        );
    }

    #[test]
    fn a_blank_flag_is_not_a_choice() {
        assert_eq!(
            resolve_child(Some("  "), Some("from-config")),
            Some("from-config".to_owned())
        );
        assert_eq!(resolve_child(Some("  "), None), None);
    }

    #[test]
    fn nothing_anywhere_means_the_command_has_to_ask() {
        assert_eq!(resolve_child(None, None), None);
    }

    #[test]
    fn the_refusal_names_both_ways_to_answer_it() {
        let message = not_signed_in_message();
        assert!(message.contains("auth login"), "{message}");
        assert!(message.contains(EMAIL_ENV), "{message}");
        assert!(message.contains(PASSWORD_ENV), "{message}");
    }

    #[test]
    fn the_refusal_names_the_command_as_it_was_invoked() {
        // Installed under another name, the hint has to follow.
        assert!(sign_in_hint("hb").contains("`hb auth login`"));
        assert!(sign_in_hint("baby").contains("`baby auth login`"));
    }

    #[test]
    fn both_files_are_named_when_somebody_asks_where_they_are() {
        let paths = describe_paths(
            Path::new("/c/config.toml"),
            Path::new("/c/credentials.toml"),
        );
        assert_eq!(paths[0].0, "config");
        assert_eq!(paths[1].1, "/c/credentials.toml");
    }
}

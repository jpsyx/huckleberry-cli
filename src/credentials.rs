//! The account file: an email, a password, and a saved session.
//!
//! These are secrets and they are kept apart from the settings, in
//! `credentials.toml` beside `config.toml`, written with mode `0600` so that
//! nobody but the owner can read it. Keeping them out of `config.toml` is what
//! makes that file safe to show somebody, paste into a bug report, or check
//! into a dotfiles repository.
//!
//! The environment wins over the file. `HUCKLEBERRY_EMAIL` and
//! `HUCKLEBERRY_PASSWORD` are how a CI job or a password manager supplies
//! credentials without any of this ever reaching a disk, and when the password
//! comes from the environment it is never written to one.
//!
//! The saved session holds a refresh token, which is as good as the password
//! for reading the account. It is stored so that a person is not asked for
//! their password on every single command; `auth logout` removes it.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use huckleberry_api::Session;
use serde::{Deserialize, Serialize};

/// The file name beside `config.toml`.
const FILE_NAME: &str = "credentials.toml";

/// The environment variable that supplies an email address.
pub const EMAIL_ENV: &str = "HUCKLEBERRY_EMAIL";

/// The environment variable that supplies a password.
pub const PASSWORD_ENV: &str = "HUCKLEBERRY_PASSWORD";

/// What is on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stored {
    /// The account's email address.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
    /// The account's password. Absent when it comes from the environment
    /// instead, which is the way to keep it off the disk entirely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// The last session, so the next command does not have to sign in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<Session>,
}

impl Stored {
    /// Whether there is anything worth writing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.email.is_empty() && self.password.is_none() && self.session.is_none()
    }
}

/// What the tool will actually use, after the environment has had its say.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Resolved {
    /// The email address, from the environment or the file.
    pub email: Option<String>,
    /// The password, from the environment or the file.
    pub password: Option<String>,
    /// Where the password came from, which decides whether a new one is
    /// written back.
    pub password_from_environment: bool,
    /// The saved session.
    pub session: Option<Session>,
}

impl Resolved {
    /// Whether there is enough here to sign in without asking anybody
    /// anything.
    #[must_use]
    pub const fn can_sign_in(&self) -> bool {
        self.email.is_some() && self.password.is_some()
    }

    /// Whether there is enough here to make a request at all: either a live
    /// session to renew, or credentials to sign in with.
    #[must_use]
    pub const fn is_usable(&self) -> bool {
        self.session.is_some() || self.can_sign_in()
    }
}

/// Applies the environment to what was on disk.
///
/// Pure, so the precedence is tested rather than reasoned about: the
/// environment wins, an empty variable counts as unset (an exported-but-blank
/// variable is a common accident and should not blank out a working
/// password), and a password that came from the environment is marked so that
/// [`save`] leaves it out of the file.
#[must_use]
pub fn resolve(stored: &Stored, email_env: Option<&str>, password_env: Option<&str>) -> Resolved {
    let email_env = email_env.map(str::trim).filter(|value| !value.is_empty());
    let password_env = password_env.filter(|value| !value.is_empty());
    Resolved {
        email: email_env
            .map(ToOwned::to_owned)
            .or_else(|| non_empty(&stored.email)),
        password: password_env
            .map(ToOwned::to_owned)
            .or_else(|| stored.password.clone()),
        password_from_environment: password_env.is_some(),
        session: stored.session.clone(),
    }
}

/// What to write back after a sign-in.
///
/// The password is written only when it did not come from the environment: a
/// caller who took the trouble to supply it that way has said where they want
/// it kept.
#[must_use]
pub fn to_store(resolved: &Resolved) -> Stored {
    Stored {
        email: resolved.email.clone().unwrap_or_default(),
        password: if resolved.password_from_environment {
            None
        } else {
            resolved.password.clone()
        },
        session: resolved.session.clone(),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The credentials file beside a configuration file.
#[must_use]
pub fn path_beside(config_path: &Path) -> PathBuf {
    config_path
        .parent()
        .map_or_else(|| PathBuf::from(FILE_NAME), |parent| parent.join(FILE_NAME))
}

/// Reads the credentials file. A file that is not there means nobody has
/// signed in yet.
pub fn load(path: &Path) -> Result<Stored> {
    match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).with_context(|| format!("reading {}", path.display())),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Stored::default()),
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

/// Writes the credentials file, readable and writable by its owner alone.
///
/// The mode is set before anything is written, not after, so the secret is
/// never on disk with looser permissions even for an instant.
pub fn save(path: &Path, stored: &Stored) -> Result<()> {
    if stored.is_empty() {
        return remove(path);
    }
    let text = toml::to_string_pretty(stored).context("serializing the credentials")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    write_private(path, &text).with_context(|| format!("writing {}", path.display()))
}

/// Deletes the credentials file. A file that was not there is a success: the
/// caller asked for it to be gone, and it is.
pub fn remove(path: &Path) -> Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).with_context(|| format!("removing {}", path.display())),
    }
}

#[cfg(unix)]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, text: &str) -> std::io::Result<()> {
    // No portable equivalent of `0600` here. The file still holds a secret, so
    // this is worth knowing about rather than papering over.
    std::fs::write(path, text)
}

#[cfg(test)]
mod precedence {
    use super::*;

    fn stored() -> Stored {
        Stored {
            email: "file@example.com".to_owned(),
            password: Some("from-the-file".to_owned()),
            session: None,
        }
    }

    #[test]
    fn the_file_is_used_when_the_environment_says_nothing() {
        let resolved = resolve(&stored(), None, None);
        assert_eq!(resolved.email.as_deref(), Some("file@example.com"));
        assert_eq!(resolved.password.as_deref(), Some("from-the-file"));
        assert!(!resolved.password_from_environment);
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let resolved = resolve(&stored(), Some("env@example.com"), Some("from-the-env"));
        assert_eq!(resolved.email.as_deref(), Some("env@example.com"));
        assert_eq!(resolved.password.as_deref(), Some("from-the-env"));
        assert!(resolved.password_from_environment);
    }

    #[test]
    fn an_exported_but_blank_variable_does_not_blank_out_a_working_password() {
        let resolved = resolve(&stored(), Some("  "), Some(""));
        assert_eq!(resolved.email.as_deref(), Some("file@example.com"));
        assert_eq!(resolved.password.as_deref(), Some("from-the-file"));
    }

    #[test]
    fn nothing_anywhere_is_nothing_rather_than_an_empty_string() {
        let resolved = resolve(&Stored::default(), None, None);
        assert_eq!(resolved.email, None);
        assert!(!resolved.can_sign_in());
        assert!(!resolved.is_usable());
    }

    #[test]
    fn a_saved_session_alone_is_enough_to_make_a_request() {
        let with_session = Stored {
            email: String::new(),
            password: None,
            session: Some(Session {
                id_token: "id".to_owned(),
                refresh_token: "refresh".to_owned(),
                user_uid: "uid".to_owned(),
                expires_at: 0,
            }),
        };
        let resolved = resolve(&with_session, None, None);
        assert!(!resolved.can_sign_in());
        assert!(resolved.is_usable(), "the refresh token can still be spent");
    }
}

#[cfg(test)]
mod writing_back {
    use super::*;

    #[test]
    fn a_password_from_the_environment_is_never_written_to_the_disk() {
        let resolved = Resolved {
            email: Some("a@b.c".to_owned()),
            password: Some("from-the-env".to_owned()),
            password_from_environment: true,
            session: None,
        };
        assert_eq!(to_store(&resolved).password, None);
    }

    #[test]
    fn a_password_the_person_typed_is_kept_so_they_are_not_asked_again() {
        let resolved = Resolved {
            email: Some("a@b.c".to_owned()),
            password: Some("typed".to_owned()),
            password_from_environment: false,
            session: None,
        };
        assert_eq!(to_store(&resolved).password.as_deref(), Some("typed"));
    }

    #[test]
    fn the_file_sits_beside_the_configuration_it_belongs_to() {
        let path = path_beside(Path::new("/home/ada/.config/huckleberry-cli/config.toml"));
        assert_eq!(
            path,
            PathBuf::from("/home/ada/.config/huckleberry-cli/credentials.toml")
        );
    }

    #[test]
    fn an_empty_store_knows_it_has_nothing_to_write() {
        assert!(Stored::default().is_empty());
        assert!(
            !to_store(&Resolved {
                email: Some("a@b.c".to_owned()),
                ..Resolved::default()
            })
            .is_empty()
        );
    }

    #[test]
    fn what_is_written_is_what_is_read_back() {
        let stored = Stored {
            email: "a@b.c".to_owned(),
            password: Some("hunter2".to_owned()),
            session: Some(Session {
                id_token: "id".to_owned(),
                refresh_token: "refresh".to_owned(),
                user_uid: "uid".to_owned(),
                expires_at: 4_600,
            }),
        };
        let text = toml::to_string_pretty(&stored).expect("serializing");
        assert_eq!(toml::from_str::<Stored>(&text).expect("parsing"), stored);
    }
}

#[cfg(test)]
mod on_disk {
    use super::*;

    /// A scratch directory that cleans up after itself, so these tests never
    /// go near a real credentials file.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "huckleberry-cli-test-{name}-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("a scratch directory");
            Self(path)
        }

        fn file(&self) -> PathBuf {
            self.0.join(FILE_NAME)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_file_that_is_not_there_means_nobody_has_signed_in() {
        let scratch = Scratch::new("absent");
        assert_eq!(load(&scratch.file()).expect("a read"), Stored::default());
    }

    #[test]
    fn a_saved_file_reads_back() {
        let scratch = Scratch::new("round-trip");
        let stored = Stored {
            email: "a@b.c".to_owned(),
            password: Some("hunter2".to_owned()),
            session: None,
        };
        save(&scratch.file(), &stored).expect("a write");
        assert_eq!(load(&scratch.file()).expect("a read"), stored);
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;

        let scratch = Scratch::new("permissions");
        save(
            &scratch.file(),
            &Stored {
                email: "a@b.c".to_owned(),
                password: Some("hunter2".to_owned()),
                session: None,
            },
        )
        .expect("a write");
        let mode = std::fs::metadata(scratch.file())
            .expect("the file")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600, "got {mode:o}");
    }

    #[test]
    fn saving_nothing_removes_the_file_rather_than_leaving_an_empty_one() {
        let scratch = Scratch::new("empty");
        save(
            &scratch.file(),
            &Stored {
                email: "a@b.c".to_owned(),
                password: None,
                session: None,
            },
        )
        .expect("a write");
        assert!(scratch.file().exists());
        save(&scratch.file(), &Stored::default()).expect("a write");
        assert!(!scratch.file().exists());
    }

    #[test]
    fn removing_a_file_that_is_already_gone_is_a_success() {
        let scratch = Scratch::new("remove");
        assert!(remove(&scratch.file()).is_ok());
    }
}

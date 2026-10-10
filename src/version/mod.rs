//! Software release information, separate from child data and Huckleberry access.

pub mod client;

use std::cmp::Ordering;

use anyhow::{Context, Result};
use semver::Version;

/// What a completed check establishes about this build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    /// Installed and published versions have the same precedence.
    UpToDate,
    /// GitHub has a newer version.
    UpdateAvailable,
    /// This build is newer than GitHub's latest release.
    Ahead,
    /// The caller explicitly disabled networking.
    Offline,
    /// GitHub has no published release.
    NoRelease,
    /// The request or response could not establish a version.
    Unavailable,
}

impl UpdateStatus {
    /// Stable machine spelling used by `info --check-update`.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UpToDate => "up_to_date",
            Self::UpdateAvailable => "update_available",
            Self::Ahead => "ahead",
            Self::Offline => "offline",
            Self::NoRelease => "no_release",
            Self::Unavailable => "unavailable",
        }
    }

    /// Human wording shared by the command and modal.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::UpToDate => "Up to date",
            Self::UpdateAvailable => "Update available",
            Self::Ahead => "Ahead of the latest release",
            Self::Offline => "Offline: update check disabled",
            Self::NoRelease => "No published release found",
            Self::Unavailable => "Unable to check GitHub",
        }
    }
}

/// A completed check, including honest unknown/offline outcomes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionReport {
    /// Version compiled into the application.
    pub installed: String,
    /// Normalized published version, when known.
    pub latest: Option<String>,
    /// How the two versions compare or why comparison was unavailable.
    pub status: UpdateStatus,
    /// A concise diagnostic, without raw server content.
    pub reason: Option<String>,
}

impl VersionReport {
    /// A deliberate offline result, with no attempted request.
    #[must_use]
    pub fn offline(installed: &str) -> Self {
        Self::without_latest(installed, UpdateStatus::Offline)
    }

    /// A failed check must never imply the installed version is current.
    #[must_use]
    pub fn unavailable(installed: &str, reason: &str) -> Self {
        Self {
            reason: Some(reason.into()),
            ..Self::without_latest(installed, UpdateStatus::Unavailable)
        }
    }

    pub(super) fn without_latest(installed: &str, status: UpdateStatus) -> Self {
        Self {
            installed: installed.into(),
            latest: None,
            status,
            reason: None,
        }
    }
}

/// Compare SemVer precedence, ignoring build metadata and a tag's leading `v`.
pub fn compare_versions(installed: &str, latest_tag: &str) -> Result<VersionReport> {
    let current = Version::parse(installed).context("invalid installed version")?;
    let latest = Version::parse(latest_tag.strip_prefix('v').unwrap_or(latest_tag))
        .context("invalid published version")?;
    let status = match current.cmp_precedence(&latest) {
        Ordering::Less => UpdateStatus::UpdateAvailable,
        Ordering::Equal => UpdateStatus::UpToDate,
        Ordering::Greater => UpdateStatus::Ahead,
    };
    Ok(VersionReport {
        installed: current.to_string(),
        latest: Some(latest.to_string()),
        status,
        reason: None,
    })
}

/// Check this build against the public repository, respecting explicit offline mode.
pub async fn check(offline: bool) -> VersionReport {
    let installed = env!("CARGO_PKG_VERSION");
    if offline {
        return VersionReport::offline(installed);
    }
    match client::GitHubClient::new(
        "https://api.github.com/repos/jpsyx/huckleberry-cli/releases/latest",
        std::time::Duration::from_secs(5),
    ) {
        Ok(client) => client.check(installed, false).await,
        Err(_) => VersionReport::unavailable(installed, "Could not initialize the GitHub client"),
    }
}

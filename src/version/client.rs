//! Public GitHub release transport; it knows nothing about Huckleberry credentials.
use std::time::Duration;

use anyhow::Result;
use reqwest::{Client, StatusCode};
use serde::Deserialize;

use super::{UpdateStatus, VersionReport, compare_versions};

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
}

/// A bounded HTTP client. The endpoint is injectable for local request tests.
pub struct GitHubClient {
    client: Client,
    endpoint: String,
}

impl GitHubClient {
    /// Build a client with a total timeout, including reading the response body.
    pub fn new(endpoint: &str, timeout: Duration) -> Result<Self> {
        Ok(Self {
            client: Client::builder().timeout(timeout).build()?,
            endpoint: endpoint.into(),
        })
    }

    /// Read the latest public release, returning a report even when GitHub is unavailable.
    pub async fn check(&self, installed: &str, offline: bool) -> VersionReport {
        if offline {
            return VersionReport::offline(installed);
        }
        let response = self
            .client
            .get(&self.endpoint)
            .header("Accept", "application/vnd.github+json")
            .header("User-Agent", format!("huckleberry-cli/{installed}"))
            .send()
            .await;
        match response {
            Ok(response) => report(response, installed).await,
            Err(error) => VersionReport::unavailable(
                installed,
                if error.is_timeout() {
                    "Request timed out"
                } else {
                    "Could not reach GitHub"
                },
            ),
        }
    }
}

async fn report(response: reqwest::Response, installed: &str) -> VersionReport {
    if response.status() == StatusCode::NOT_FOUND {
        return VersionReport::without_latest(installed, UpdateStatus::NoRelease);
    }
    if !response.status().is_success() {
        return VersionReport::unavailable(
            installed,
            &format!("GitHub returned HTTP {}", response.status().as_u16()),
        );
    }
    let release = match response.json::<Release>().await {
        Ok(release) => release,
        Err(error) => {
            return VersionReport::unavailable(
                installed,
                if error.is_timeout() {
                    "Request timed out"
                } else {
                    "GitHub returned an invalid release"
                },
            );
        }
    };
    if release.draft || release.prerelease {
        return VersionReport::unavailable(
            installed,
            "GitHub returned an unpublished or prerelease version",
        );
    }
    compare_versions(installed, &release.tag_name).unwrap_or_else(|_| {
        VersionReport::unavailable(installed, "GitHub returned an invalid version")
    })
}

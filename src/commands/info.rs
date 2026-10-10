//! `info`: what this build is.

use anyhow::{Result, bail};

use crate::session::Context;

/// Shows build details as a table, or stable `key=value` lines on a pipe.
pub async fn run(context: &Context, check_update: bool) -> Result<()> {
    let mut machine = facts(
        &context.config_path.display().to_string(),
        &context.credentials_path.display().to_string(),
        &context.config.timezone,
        context.config.child(),
    );
    let mut fields = fields(context);
    let report = if check_update {
        Some(crate::version::check(context.offline.is_some()).await)
    } else {
        None
    };
    if let Some(report) = &report {
        let latest = report.latest.as_deref().unwrap_or("");
        machine.extend([
            format!("latest_version={latest}"),
            format!("update_status={}", report.status.code()),
        ]);
        fields.extend([
            (
                "Latest version",
                report.latest.clone().unwrap_or_else(|| "Unknown".into()),
            ),
            ("Update status", report.status.message().into()),
        ]);
    }
    context.present("🍼 Huckleberry", &fields, &machine);
    if let Some(report) = report
        && report.status == crate::version::UpdateStatus::Unavailable
    {
        bail!(
            "{}: {}",
            report.status.message(),
            report.reason.as_deref().unwrap_or("Unknown error")
        );
    }
    Ok(())
}

fn fields(context: &Context) -> Vec<(&'static str, String)> {
    vec![
        ("App", crate::APP_NAME.into()),
        ("Version", env!("CARGO_PKG_VERSION").into()),
        ("API version", huckleberry_api::VERSION.into()),
        ("Settings file", context.config_path.display().to_string()),
        (
            "Credentials file",
            context.credentials_path.display().to_string(),
        ),
        ("Timezone", context.config.timezone.clone()),
        (
            "Child",
            context.config.child().unwrap_or("Not selected").into(),
        ),
    ]
}

/// The facts and their order.
#[must_use]
pub fn facts(
    config_path: &str,
    credentials_path: &str,
    timezone: &str,
    child: Option<&str>,
) -> Vec<String> {
    vec![
        format!("name={}", crate::APP_NAME),
        format!("version={}", env!("CARGO_PKG_VERSION")),
        format!("api_version={}", huckleberry_api::VERSION),
        format!("config={config_path}"),
        format!("credentials={credentials_path}"),
        format!("timezone={timezone}"),
        format!("child={}", child.unwrap_or("")),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_facts_include_the_version_of_both_halves() {
        let shown = facts("/c/config.toml", "/c/credentials.toml", "UTC", Some("abc"));
        assert!(shown.iter().any(|line| line.starts_with("version=")));
        assert!(shown.iter().any(|line| line.starts_with("api_version=")));
    }

    #[test]
    fn every_fact_is_one_key_value_line() {
        for line in facts("/c", "/d", "UTC", None) {
            assert!(line.contains('='), "{line}");
            assert!(!line.contains('\n'), "{line}");
        }
    }

    #[test]
    fn no_credential_is_ever_printed_only_where_it_lives() {
        let shown = facts("/c/config.toml", "/c/credentials.toml", "UTC", Some("abc")).join("\n");
        assert!(shown.contains("credentials=/c/credentials.toml"));
        for secret in ["password", "token", "email"] {
            assert!(!shown.contains(&format!("{secret}=")), "{shown}");
        }
    }
}

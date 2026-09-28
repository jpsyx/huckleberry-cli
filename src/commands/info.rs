//! `info`: what this build is.

use anyhow::Result;

use crate::session::Context;

/// Prints the facts, one `key=value` line each.
pub fn run(context: &Context) -> Result<()> {
    crate::render::print(&facts(
        &context.config_path.display().to_string(),
        &context.credentials_path.display().to_string(),
        &context.config.timezone,
        context.config.child(),
    ));
    Ok(())
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

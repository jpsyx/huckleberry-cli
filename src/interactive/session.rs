//! Session-scoped command overrides, separate from persistent settings.
use crate::cli::Cli;
use std::path::PathBuf;

/// Global flags applied to subsequent interactive commands.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionOptions {
    /// Alternate settings and credential location.
    pub config: Option<PathBuf>,
    /// Temporary child override.
    pub child: Option<String>,
    /// Snapshot source; None selects live data.
    pub offline: Option<PathBuf>,
    /// Diagnostic output.
    pub verbose: bool,
}
impl SessionOptions {
    /// Captures global values from the invocation.
    #[must_use]
    pub fn from_cli(cli: &Cli) -> Self {
        Self {
            config: cli.config.clone(),
            child: cli.child.clone(),
            offline: cli.offline.clone(),
            verbose: cli.verbose,
        }
    }
}

/// Chooses an account child by name without persisting an override.
pub async fn choose_child(context: &crate::session::Context) -> anyhow::Result<String> {
    let client = context.client()?;
    let user = client.user().await?;
    crate::commands::persist_session(context, &client).await?;
    let items = user
        .child_list
        .iter()
        .map(|child| crate::prompt::select::MenuItem {
            label: child.nickname.clone().unwrap_or_else(|| child.cid.clone()),
            detail: Some(child.cid.clone()),
        })
        .collect::<Vec<_>>();
    let index = crate::prompt::select::choose("Which child?", &items, 0, context.theme)?;
    Ok(user.child_list[index].cid.clone())
}

//! Transactional menus for temporary command overrides.
use super::session::{SessionOptions, choose_child, finish_edit};
use crate::{
    prompt::{
        self, Question,
        select::{self, MenuItem},
    },
    theme::Theme,
};
use anyhow::Result;

/// Edits a clone; Escape at the outer menu discards the whole draft.
pub async fn edit_options(current: &SessionOptions, theme: Theme) -> Result<SessionOptions> {
    let mut pending = current.clone();
    loop {
        let labels = session_labels(&pending);
        let index = match choose("Session options", &labels, 0, theme) {
            Ok(index) => index,
            Err(error) if prompt::is_cancelled(&error) => {
                return Ok(finish_edit(current, pending, false));
            }
            Err(error) => return Err(error),
        };
        let result = match index {
            0 => return Ok(finish_edit(current, pending, true)),
            1 => prompt::confirm("Verbose diagnostics?", pending.verbose, theme)
                .map(|value| pending.verbose = value),
            2 => path(
                &mut pending.config,
                "Configuration file",
                "Use default configuration",
                theme,
            ),
            3 => child(&mut pending, theme).await,
            4 => path(
                &mut pending.offline,
                "Data source",
                "Live Huckleberry data",
                theme,
            ),
            _ => return Ok(finish_edit(current, pending, false)),
        };
        if let Err(error) = result {
            if !prompt::is_cancelled(&error) {
                crate::render::note(&format!("{error:#}"));
            }
        }
    }
}

fn path_label(path: Option<&std::path::Path>, fallback: &str) -> String {
    path.map_or_else(|| fallback.into(), |path| path.display().to_string())
}

fn path(
    value: &mut Option<std::path::PathBuf>,
    title: &str,
    clear: &str,
    theme: Theme,
) -> Result<()> {
    match choose(
        title,
        &["Keep current".into(), clear.into(), "Enter a path".into()],
        0,
        theme,
    )? {
        1 => *value = None,
        2 => {
            *value = Some(
                prompt::ask(
                    &Question::new("path", "Path?", "--config or --offline"),
                    theme,
                )?
                .into(),
            );
        }
        _ => {}
    }
    Ok(())
}

async fn child(pending: &mut SessionOptions, theme: Theme) -> Result<()> {
    match choose(
        "Child override",
        &[
            "Keep current".into(),
            "Use configured child".into(),
            "Choose a child by name".into(),
            "Enter child ID".into(),
        ],
        0,
        theme,
    )? {
        1 => pending.child = None,
        2 => {
            let context = super::session::load_context(pending, theme)?;
            pending.child = Some(choose_child(&context).await?);
        }
        3 => {
            pending.child = Some(prompt::ask(
                &Question::new("child", "Child ID?", "--child <CID>"),
                theme,
            )?);
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn choose(
    title: &str,
    labels: &[String],
    default: usize,
    theme: Theme,
) -> Result<usize> {
    let items = labels
        .iter()
        .map(|label| MenuItem {
            label: label.clone(),
            detail: None,
        })
        .collect::<Vec<_>>();
    select::choose(title, &items, default, theme)
}

fn session_labels(pending: &SessionOptions) -> [String; 6] {
    [
        "Apply session options".to_owned(),
        format!("Verbose: {}", pending.verbose),
        format!(
            "Configuration: {}",
            path_label(pending.config.as_deref(), "default")
        ),
        format!(
            "Child: {}",
            pending.child.as_deref().unwrap_or("configured child")
        ),
        format!("Data: {}", path_label(pending.offline.as_deref(), "live")),
        "Discard changes".into(),
    ]
}

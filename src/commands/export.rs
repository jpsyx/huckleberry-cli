//! `export`: everything, as JSON.
//!
//! Stdout by default, because that is where data goes and because piping this
//! into `jq` is the point. `--out` writes a file, and that file is what
//! `--offline` reads back on any other command.

use std::path::Path;

use anyhow::Result;

use crate::session::Context;

/// Pulls a window and writes it out.
pub async fn run(context: &Context, days: Option<u32>, out: Option<&Path>) -> Result<()> {
    let (dataset, _) = super::load(context, days).await?;
    let text = crate::dataset::render_snapshot(&dataset)?;

    match out.filter(|path| path.as_os_str() != "-") {
        None => crate::render::print(std::slice::from_ref(&text)),
        Some(path) => {
            crate::dataset::write_snapshot(path, &dataset)?;
            context.receipt(
                "📁 History exported",
                &[
                    ("File", path.display().to_string()),
                    ("Records", summary_line(&dataset)),
                ],
                &[],
            );
        }
    }
    Ok(())
}

/// What went into the snapshot, as a line for `--verbose`.
#[must_use]
pub fn summary_line(dataset: &crate::domain::types::Dataset) -> String {
    let counts = [
        ("sleeps", dataset.sleep.len()),
        ("feeds", dataset.feeds.len()),
        ("diapers", dataset.diapers.len()),
        ("pumps", dataset.pumps.len()),
        ("milestones", dataset.milestones.len()),
    ];
    let parts: Vec<String> = counts
        .into_iter()
        .filter(|(_, count)| *count > 0)
        .map(|(name, count)| format!("{count} {name}"))
        .collect();
    if parts.is_empty() {
        return "nothing in this window".to_owned();
    }
    parts.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, sleep};

    #[test]
    fn the_summary_names_only_what_is_there() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        data.sleep = vec![sleep(AFTERNOON - 7_200.0, 3_600.0)];
        let line = summary_line(&data);
        assert!(line.contains("1 feeds"), "{line}");
        assert!(line.contains("1 sleeps"), "{line}");
        assert!(!line.contains("diapers"), "{line}");
    }

    #[test]
    fn an_empty_window_says_so_rather_than_listing_zeroes() {
        assert_eq!(summary_line(&dataset()), "nothing in this window");
    }
}

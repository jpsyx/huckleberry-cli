//! `foods`: the family's own foods and Huckleberry's curated list.

use anyhow::Result;

use crate::cli::FoodsAction;
use crate::listing::{Column, Listing, Role, Row};
use crate::prompt::{self, Question};
use crate::render::format;
use crate::session::Context;
use crate::theme::Tone;

/// Runs the chosen action.
pub async fn run(context: &Context, action: &FoodsAction) -> Result<()> {
    match action {
        FoodsAction::List {
            custom,
            curated,
            search,
            archived,
        } => list(context, *custom, *curated, search.as_deref(), *archived).await,
        FoodsAction::Add { name } => add(context, name.clone()).await,
    }
}

/// Lists foods, one `source\tname` line each.
async fn list(
    context: &Context,
    custom_only: bool,
    curated_only: bool,
    search: Option<&str>,
    archived: bool,
) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;

    let mut rows = Vec::new();
    if !curated_only {
        for food in client.custom_foods(&cid, archived).await? {
            let state = if food.archived { "archived" } else { "" };
            rows.push(
                Row::new(
                    food.name.clone(),
                    [food.name, "Family food".to_owned(), state.to_owned()],
                )
                .group("Your foods")
                .tone(Tone::Feeding),
            );
        }
    }
    if !custom_only {
        context.detail("downloading the curated food database");
        for food in client.curated_foods().await? {
            let flags: Vec<&str> = [
                food.is_common_allergen
                    .unwrap_or(false)
                    .then_some("common allergen"),
                food.is_high_choking_hazard
                    .unwrap_or(false)
                    .then_some("choking hazard"),
            ]
            .into_iter()
            .flatten()
            .collect();
            rows.push(
                Row::new(
                    food.name.clone(),
                    [food.name, "Food database".to_owned(), flags.join(", ")],
                )
                .group("Huckleberry's foods")
                .tone(if flags.is_empty() {
                    Tone::Accent
                } else {
                    Tone::Attention
                }),
            );
        }
    }
    super::persist_session(context, &client).await?;

    // Hundreds of curated foods is the listing this tool most needs a search
    // in, so `/` is where the filtering happens and `--search` is the same
    // thing for a script.
    Listing::new("🥑 Foods", "foods", &COLUMNS)
        .rows(rows)
        .group_heading(str::to_owned)
        .empty("no foods")
        .query(search)
        .verb("↑/↓ j/k w/s move · / searches · h/a or q leaves")
        .show(context.output_theme())
}

/// What a food listing shows.
const COLUMNS: [Column; 3] = [
    Column::new("food", Role::Value),
    Column::new("source", Role::Kind),
    Column::new("notes", Role::Muted),
];

/// Adds a food to the family's own list.
async fn add(context: &Context, name: Option<String>) -> Result<()> {
    let (client, cid) = super::client_and_child(context).await?;
    let name = match name {
        Some(given) => given,
        None => prompt::ask(
            &Question::new("food name", "What is it called?", "<NAME>"),
            context.theme,
        )?,
    };

    let existing = client.custom_foods(&cid, true).await.unwrap_or_default();
    if let Some(already) = existing
        .iter()
        .find(|food| food.name.eq_ignore_ascii_case(name.trim()))
    {
        context.warn(&format!(
            "`{}` is already on the list{}.",
            already.name,
            if already.archived { " (archived)" } else { "" }
        ));
        return Ok(());
    }

    let food = client.create_custom_food(&cid, &name, "").await?;
    super::persist_session(context, &client).await?;
    context.receipt(
        "🥑 Food added",
        &[("Food", food.name.clone()), ("Food ID", food.id.clone())],
        &[format!("{}\t{}", food.id, food.name)],
    );
    Ok(())
}

/// A food's line, as `foods list` prints it.
#[must_use]
pub fn line(source: &str, name: &str, note: Option<&str>) -> String {
    let name = format::truncate(name, 48);
    note.map_or_else(
        || format!("{source}\t{name}"),
        |text| format!("{source}\t{name}\t{text}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_food_line_is_tab_separated_so_it_can_be_cut() {
        assert_eq!(line("custom", "Avocado", None), "custom\tAvocado");
        assert_eq!(
            line("curated", "Peanut", Some("common allergen")),
            "curated\tPeanut\tcommon allergen"
        );
    }

    #[test]
    fn a_very_long_name_is_cut_rather_than_wrapping_the_column() {
        let long = "a".repeat(80);
        let rendered = line("custom", &long, None);
        assert!(rendered.chars().count() < 60, "{rendered}");
        assert!(rendered.ends_with('…'));
    }
}

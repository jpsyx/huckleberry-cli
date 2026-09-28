//! `foods`: the family's own foods and Huckleberry's curated list.

use anyhow::Result;

use crate::cli::FoodsAction;
use crate::prompt::{self, Question};
use crate::render::format;
use crate::session::Context;

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
    let wanted = search.map(str::to_lowercase);
    let matches = |name: &str| {
        wanted
            .as_ref()
            .is_none_or(|needle| name.to_lowercase().contains(needle))
    };

    let mut shown = 0usize;
    let mut rows = Vec::new();
    let mut machine = Vec::new();
    if !curated_only {
        for food in client.custom_foods(&cid, archived).await? {
            if !matches(&food.name) {
                continue;
            }
            let state = if food.archived { "\tarchived" } else { "" };
            machine.push(format!("custom\t{}{state}", food.name));
            rows.push(vec![
                food.name,
                "Family food".into(),
                if food.archived { "Archived" } else { "" }.into(),
            ]);
            shown += 1;
        }
    }
    if !custom_only {
        context.detail("downloading the curated food database");
        for food in client.curated_foods().await? {
            if !matches(&food.name) {
                continue;
            }
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
            let note = if flags.is_empty() {
                String::new()
            } else {
                format!("\t{}", flags.join(", "))
            };
            machine.push(format!("curated\t{}{note}", food.name));
            rows.push(vec![food.name, "Food database".into(), flags.join(", ")]);
            shown += 1;
        }
    }
    super::persist_session(context, &client).await?;

    if shown == 0 {
        context.warn(&search.map_or_else(
            || "No foods.".to_owned(),
            |needle| format!("No food matches `{needle}`."),
        ));
    } else {
        context.table("🥑 Foods", &["Food", "Source", "Notes"], &rows, &machine);
        context.detail(&format!("{shown} foods"));
    }
    Ok(())
}

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

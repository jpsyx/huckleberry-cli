//! A shared field picker records explicit changes; Keep never normalizes stored data.
use crate::{
    edit::Draft,
    interactive::options::fields,
    prompt::{
        self, Question,
        select::{self, MenuItem},
    },
    session::Context,
};
use anyhow::Result;

/// Collects only deliberate changes, starting on Done and keeping every other field.
pub async fn collect(
    context: &Context,
    draft: Option<&Draft>,
    started: f64,
    live: bool,
    mut changes: Vec<String>,
) -> Result<Vec<String>> {
    if !prompt::available() {
        anyhow::bail!("no changes supplied: pass --set at=<TIME> or --set <FIELD=VALUE>");
    }
    let time_key = if live { "start" } else { "at" };
    let mut field_names = vec![time_key];
    if let Some(draft) = draft {
        field_names.extend(draft.fields());
    }
    loop {
        let mut labels = vec!["Done (keep other fields)".into()];
        labels.extend(field_names.iter().map(|field| {
            changes
                .iter()
                .find(|change| change.starts_with(&format!("{field}=")))
                .map_or_else(|| format!("Edit {field} (keep current)"), Clone::clone)
        }));
        let index = choose(context, "Fields to change", &labels)?;
        if index == 0 {
            return Ok(changes);
        }
        let field = field_names[index - 1];
        let value = read(context, field, draft, started).await?;
        if field != time_key
            && let (Some(value), Some(mut checked)) = (&value, draft.cloned())
        {
            checked.set(field, value)?;
        }
        fields::update_change(&mut changes, field, value.as_deref());
    }
}

async fn read(
    context: &Context,
    field: &str,
    draft: Option<&Draft>,
    started: f64,
) -> Result<Option<String>> {
    let mut labels = vec!["Keep current value".into()];
    let choices = fields::choices(field);
    labels.extend(choices.iter().map(|(_, label)| label.clone()));
    if choices.is_empty() {
        labels.push("Enter a new value".into());
    }
    if fields::optional(field) {
        labels.push("Clear value".into());
    }
    let index = choose(context, field, &labels)?;
    if index == 0 {
        return Ok(None);
    }
    if labels[index] == "Clear value" {
        return Ok(Some(String::new()));
    }
    if let Some((value, _)) = choices.get(index - 1) {
        return Ok(Some(value.clone()));
    }
    if field == "foods" {
        return food_value(context, draft).await.map(Some);
    }
    if matches!(field, "at" | "start") {
        let changed = if field == "at" {
            Some(prompt::time::read_edit_at(context, None, started)?)
        } else {
            prompt::time::read_edit_start(context, None, Some(started))?
        };
        return changed
            .filter(|changed| changed.to_bits() != started.to_bits())
            .map(|changed| Ok(context.calendar()?.zoned(changed).datetime().to_string()))
            .transpose();
    }
    let question = Question::new("field value", "New value?", "--set <KEY=VALUE>");
    Ok(Some(prompt::ask(&question, context.theme)?))
}

async fn food_value(context: &Context, draft: Option<&Draft>) -> Result<String> {
    let current = match draft {
        Some(Draft::Solids(meal)) => meal.foods.clone(),
        _ => vec![],
    };
    let (client, cid) = crate::commands::client_and_child(context).await?;
    Ok(
        crate::commands::feed::solids::ask_for_foods(context, &client, &cid, &current)
            .await?
            .join(", "),
    )
}

fn choose(context: &Context, title: &str, labels: &[String]) -> Result<usize> {
    let items = labels
        .iter()
        .map(|label| MenuItem {
            label: label.clone(),
            detail: None,
        })
        .collect::<Vec<_>>();
    select::choose(title, &items, 0, context.theme)
}

/// Compatibility entry for callers collecting a complete history form.
pub async fn fill(
    context: &Context,
    _client: &huckleberry_api::Huckleberry,
    _cid: &str,
    draft: &mut Option<Draft>,
    started: &mut f64,
) -> Result<()> {
    let changes = collect(context, draft.as_ref(), *started, false, vec![]).await?;
    for change in changes {
        let (key, value) = change.split_once('=').expect("collected field change");
        if key == "at" {
            *started = prompt::time::read_edit_at(context, Some(value), *started)?;
        } else if let Some(draft) = draft {
            draft.set(key, value)?;
        }
    }
    Ok(())
}

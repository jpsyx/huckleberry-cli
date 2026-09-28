//! Builds repeated field changes without typing field names or KEY=VALUE.
use super::{fields, value::choose};
use crate::{
    interactive::draft::CommandDraft,
    prompt::{self, Question},
    session::Context,
};
use anyhow::Result;

pub(super) async fn collect(context: &Context, pending: &mut CommandDraft) -> Result<()> {
    let selected = crate::commands::edit::pick::select(
        context,
        pending
            .values
            .get("id")
            .and_then(|values| values.first())
            .map(String::as_str),
        pending
            .values
            .get("days")
            .and_then(|values| values.first())
            .and_then(|value| value.parse().ok()),
        pending
            .values
            .get("limit")
            .and_then(|values| values.first())
            .and_then(|value| value.parse().ok())
            .unwrap_or(40),
    )
    .await?;
    let draft = selected.draft;
    let time_key = if selected.token == "sleep/current" {
        "start"
    } else {
        "at"
    };
    let mut fields = vec![time_key];
    if let Some(draft) = &draft {
        fields.extend(draft.fields());
    }
    let mut changes = pending.values.get("set").cloned().unwrap_or_default();
    loop {
        let mut labels = vec!["Done".into()];
        labels.extend(fields.iter().map(|field| {
            let value = changes
                .iter()
                .find(|change| change.starts_with(&format!("{field}=")));
            value.map_or_else(|| format!("Edit {field} (keep current)"), Clone::clone)
        }));
        let index = choose(context, "Fields to change", &labels, 0)?;
        if index == 0 {
            pending.set("id", vec![selected.token]);
            pending.set("set", changes);
            return Ok(());
        }
        let field = fields[index - 1];
        let value = read(context, field, draft.as_ref(), selected.started).await?;
        if field != time_key
            && let (Some(value), Some(mut checked)) = (&value, draft.clone())
        {
            checked.set(field, value)?;
        }
        fields::update_change(&mut changes, field, value.as_deref());
    }
}

async fn read(
    context: &Context,
    field: &str,
    draft: Option<&crate::edit::Draft>,
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
    let index = choose(context, field, &labels, 0)?;
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
        let current = match draft {
            Some(crate::edit::Draft::Solids(meal)) => meal.foods.clone(),
            _ => vec![],
        };
        let (client, cid) = crate::commands::client_and_child(context).await?;
        return Ok(Some(
            crate::commands::feed::solids::ask_for_foods(context, &client, &cid, &current)
                .await?
                .join(", "),
        ));
    }
    let shown = context
        .calendar()?
        .zoned(started)
        .strftime("%Y-%m-%d %-I:%M %p")
        .to_string();
    let mut question = Question::new("field value", "New value?", "--set <KEY=VALUE>");
    if field == "at" {
        question = question.with_default(&shown);
    }
    Ok(Some(prompt::ask(&question, context.theme)?))
}

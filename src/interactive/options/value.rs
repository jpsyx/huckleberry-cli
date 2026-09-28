//! Scalar and repeated command-option controls.
use super::{OptionBinding, OptionControl, set_value};
use crate::interactive::{catalog::metadata, draft::CommandDraft};
use crate::{
    prompt::{
        self, Question,
        select::{self, MenuItem},
    },
    session::Context,
};
use anyhow::{Result, anyhow};

pub(super) async fn edit(
    context: &Context,
    draft: &mut CommandDraft,
    binding: &OptionBinding,
) -> Result<()> {
    let command = metadata(&draft.path).ok_or_else(|| anyhow!("unknown command"))?;
    let arg = command
        .get_arguments()
        .find(|arg| arg.get_id().as_str() == binding.argument_id)
        .ok_or_else(|| anyhow!("unknown option"))?;
    let title = arg
        .get_help()
        .map_or_else(|| binding.argument_id.clone(), ToString::to_string);
    let mut labels = vec![
        "Keep current option".into(),
        "Use default / ask when running".into(),
    ];
    let values = arg
        .get_possible_values()
        .into_iter()
        .map(|value| value.get_name().to_owned())
        .collect::<Vec<_>>();
    if binding.control == OptionControl::Boolean {
        labels.extend(["Yes".into(), "No".into()]);
    } else if values.is_empty() {
        labels.push("Enter a value".into());
    } else {
        labels.extend(
            values
                .iter()
                .map(|value| crate::render::output::words(value)),
        );
    }
    if binding.control == OptionControl::Repeated {
        return repeated(context, draft, binding).await;
    }
    if binding.argument_id == "cid" {
        labels.push("Choose a child".into());
    }
    if binding.argument_id == "tracker" {
        labels.extend(
            [
                "health",
                "pump",
                "milestones",
                "diaper",
                "feed",
                "sleep",
                "solids",
            ]
            .map(str::to_owned),
        );
    }
    let index = choose(context, &title, &labels, 0)?;
    match index {
        0 => {}
        1 => draft.clear(&binding.argument_id),
        _ => {
            let value = scalar(context, binding, &values, &labels, index).await?;
            set_value(draft, &binding.argument_id, vec![value]);
        }
    }
    Ok(())
}

async fn scalar(
    context: &Context,
    binding: &OptionBinding,
    values: &[String],
    labels: &[String],
    index: usize,
) -> Result<String> {
    if binding.control == OptionControl::Boolean {
        return Ok((index == 2).to_string());
    }
    if let Some(value) = values.get(index - 2) {
        return Ok(value.clone());
    }
    if labels[index] == "Choose a child" {
        return crate::interactive::session::choose_child(context).await;
    }
    if binding.argument_id == "tracker" && index > 2 {
        return Ok(labels[index].clone());
    }
    let question = Question::new("option value", "Value?", "the corresponding command flag");
    if binding.control == OptionControl::Secret {
        prompt::ask_secret(&question, context.theme)
    } else {
        prompt::ask(&question, context.theme)
    }
}

async fn repeated(
    context: &Context,
    draft: &mut CommandDraft,
    binding: &OptionBinding,
) -> Result<()> {
    if binding.argument_id == "foods" {
        let (client, cid) = crate::commands::client_and_child(context).await?;
        let current = draft.values.get("foods").cloned().unwrap_or_default();
        let foods =
            crate::commands::feed::solids::ask_for_foods(context, &client, &cid, &current).await?;
        draft.set("foods", foods);
        return Ok(());
    }
    loop {
        let mut labels = vec![
            "Done".into(),
            "Add a value".into(),
            "Clear overrides".into(),
        ];
        let current = draft
            .values
            .get(&binding.argument_id)
            .cloned()
            .unwrap_or_default();
        labels.extend(current.iter().map(|value| format!("Remove {value}")));
        let index = choose(context, "Values", &labels, 0)?;
        match index {
            0 => return Ok(()),
            1 => {
                let mut values = current;
                values.push(prompt::ask(
                    &Question::new("value", "Value?", "--value"),
                    context.theme,
                )?);
                draft.set(&binding.argument_id, values);
            }
            2 => draft.clear(&binding.argument_id),
            _ => {
                let mut values = current;
                values.remove(index - 3);
                draft.set(&binding.argument_id, values);
            }
        }
    }
}

/// Displays owned labels using the shared selection component.
pub(super) fn choose(
    context: &Context,
    title: &str,
    labels: &[String],
    default: usize,
) -> Result<usize> {
    let items = labels
        .iter()
        .map(|label| MenuItem {
            label: label.clone(),
            detail: None,
        })
        .collect::<Vec<_>>();
    select::choose(title, &items, default, context.theme)
}

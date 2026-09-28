//! Solid foods and meal selection.
use super::{
    Choice, Context, FoodReference, Huckleberry, MealInput, Question, Reaction, Result,
    ask_for_notes, bail, format, output, prompt,
};
/// Records a meal, asking for everything that was not given.
pub(super) async fn solids(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: MealInput<'_>,
) -> Result<()> {
    let MealInput {
        foods,
        amount,
        reaction,
        notes,
        at,
    } = input;
    let at = prompt::time::read_at(context, at)?;
    let named = if foods.is_empty() {
        ask_for_foods(context, client, cid, &[]).await?
    } else {
        foods.to_vec()
    };
    let amount = match amount {
        Some(given) => given.to_owned(),
        None => ask_for_helping(context)?,
    };
    let reaction = match reaction {
        Some(given) => Some(given),
        None => ask_for_reaction(context)?,
    };
    let notes = match notes {
        Some(given) => Some(given.to_owned()),
        None => ask_for_notes(context)?,
    };

    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    let references: Vec<FoodReference> = named
        .iter()
        .map(|name| match_food(name, &known, &amount))
        .collect();

    client
        .log_solids_at(
            cid,
            &references,
            notes.as_deref(),
            reaction.map(Reaction::to_api),
            None,
            at,
        )
        .await?;
    context.receipt(
        "🥑 Meal recorded",
        &[
            ("When", format::date_time(at, &context.calendar()?)),
            ("Foods", named.join(", ")),
            ("Amount", amount),
            (
                "Reaction",
                reaction.map_or_else(
                    || "Not recorded".into(),
                    |reaction| output::words(reaction.to_api().as_str()),
                ),
            ),
            ("Notes", notes.unwrap_or_else(|| "None".into())),
        ],
        &[],
    );
    Ok(())
}

/// How much of it, in whatever words suit. "some" is what the app writes when
/// nobody says, so it is what Enter takes.
fn ask_for_helping(context: &Context) -> Result<String> {
    let choices = [
        Choice {
            value: "some",
            hint: "Some",
        },
        Choice {
            value: "custom",
            hint: "Enter an amount",
        },
    ];
    let question = Question::new("amount", "How much?", "--amount <TEXT>")
        .with_choices(&choices)
        .with_default("some");
    let selected = prompt::ask(&question, context.theme)?;
    if selected == "custom" {
        prompt::ask(
            &Question::new("amount", "How much?", "--amount <TEXT>"),
            context.theme,
        )
    } else {
        Ok(selected)
    }
}

/// How it went, which is optional: a meal nobody had an opinion about is a
/// meal, and an invented reaction is worse than none.
fn ask_for_reaction(context: &Context) -> Result<Option<Reaction>> {
    const CHOICES: [Choice<'static>; 4] = [
        Choice {
            value: "loved",
            hint: "Loved it",
        },
        Choice {
            value: "meh",
            hint: "Took it or left it",
        },
        Choice {
            value: "hated",
            hint: "Hated it",
        },
        Choice {
            value: "allergic",
            hint: "Reacted badly",
        },
    ];
    let question = Question::new("reaction", "How did it go?", "--reaction <REACTION>")
        .with_choices(&CHOICES)
        .optional();
    Ok(
        prompt::ask_optional(&question, context.theme)?.map(|answer| match answer.as_str() {
            "meh" => Reaction::Meh,
            "hated" => Reaction::Hated,
            "allergic" => Reaction::Allergic,
            _ => Reaction::Loved,
        }),
    )
}

/// Turns a typed name into a food reference, matching the family's own foods
/// by name so that logging "Avocado" twice does not create two foods.
#[must_use]
pub fn match_food(
    name: &str,
    known: &[huckleberry_api::models::solids::CustomFood],
    amount: &str,
) -> FoodReference {
    let wanted = name.trim();
    known
        .iter()
        .find(|food| food.name.eq_ignore_ascii_case(wanted))
        .map_or_else(
            // A food nobody has added yet is referenced by its own name as the
            // id. The entry still reads correctly in the app; it is simply not
            // linked to a row in the family's food list.
            || FoodReference::custom(wanted, wanted, amount),
            |food| FoodReference::custom(&food.id, &food.name, amount),
        )
}

/// The foods pending in a meal.
#[derive(Debug, Clone)]
pub struct FoodSelection {
    /// Selected food names in order.
    pub selected: Vec<String>,
}

/// One deliberate change to the selected foods.
pub enum FoodAction {
    /// Add a food by name.
    Add(String),
    /// Remove the selected position.
    Remove(usize),
    /// Finish the meal selection.
    Done,
}
impl FoodSelection {
    /// Applies a choice and reports whether a nonempty selection is finished.
    pub fn apply(&mut self, action: FoodAction) -> bool {
        match action {
            FoodAction::Add(name) => {
                if !name.trim().is_empty() && !self.selected.contains(&name) {
                    self.selected.push(name);
                }
            }
            FoodAction::Remove(index) => {
                if index < self.selected.len() {
                    self.selected.remove(index);
                }
            }
            FoodAction::Done => return !self.selected.is_empty(),
        }
        false
    }
}

/// Selects multiple known foods or enters new names; cancellation never submits.
pub async fn ask_for_foods(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    current: &[String],
) -> Result<Vec<String>> {
    if !prompt::available() {
        bail!("no foods: pass --food <NAME>");
    }
    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    let names: Vec<String> = known.into_iter().map(|food| food.name).collect();
    let mut selection = FoodSelection {
        selected: current.to_vec(),
    };
    loop {
        let items = food_items(&selection, &names);
        let title = format!("Foods: {}", selection.selected.join(", "));
        let index = prompt::select::choose(&title, &items, 0, context.theme)?;
        let action = food_action(context, index, &selection, &names)?;
        if selection.apply(action) {
            return Ok(selection.selected);
        }
    }
}

fn food_items(selection: &FoodSelection, known: &[String]) -> Vec<prompt::select::MenuItem> {
    let mut labels = if selection.selected.is_empty() {
        vec![]
    } else {
        vec!["Done".into()]
    };
    labels.extend(known.iter().map(|name| format!("Add {name}")));
    labels.push("Enter a new food name".into());
    labels.extend(
        selection
            .selected
            .iter()
            .map(|name| format!("Remove {name}")),
    );
    labels
        .into_iter()
        .map(|label| prompt::select::MenuItem {
            label,
            detail: None,
        })
        .collect()
}

fn food_action(
    context: &Context,
    index: usize,
    selection: &FoodSelection,
    known: &[String],
) -> Result<FoodAction> {
    if !selection.selected.is_empty() && index == 0 {
        return Ok(FoodAction::Done);
    }
    let index = index - usize::from(!selection.selected.is_empty());
    if let Some(name) = known.get(index) {
        return Ok(FoodAction::Add(name.clone()));
    }
    if index == known.len() {
        return Ok(FoodAction::Add(prompt::ask(
            &Question::new("food", "Food name?", "--food <NAME>"),
            context.theme,
        )?));
    }
    Ok(FoodAction::Remove(index - known.len() - 1))
}

//! Solid foods and meal selection.
use super::{
    Context, FoodReference, Huckleberry, MealInput, Reaction, Result, bail, format, output, prompt,
};
/// Records a meal, asking for everything that was not given.
pub(super) async fn solids(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: MealInput<'_>,
) -> Result<()> {
    let model = create_model(context, client, cid, input).await?;
    let answer = crate::prompt::flow::runner::read(
        model,
        huckleberry_api::client::now_seconds(),
        context.theme,
    )?;
    execute(context, client, cid, answer).await
}

pub(super) async fn create_model(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    input: MealInput<'_>,
) -> Result<super::solids_form::SolidsForm> {
    let at = prompt::time::resolve_at(
        context,
        input.at,
        prompt::available() || prompt::host::hosted(),
    )?;
    if input.foods.is_empty() && !prompt::available() && !prompt::host::hosted() {
        bail!("no foods: pass --food <NAME>");
    }
    let known = client.custom_foods(cid, false).await.unwrap_or_default();
    Ok(super::solids_form::SolidsForm::create_solids(
        context.calendar()?,
        super::solids_form::SolidsInput {
            at,
            foods: input.foods.to_vec(),
            amount: input.amount.map(str::to_owned),
            reaction: input.reaction,
            notes: input.notes.map(str::to_owned),
        },
        known,
    ))
}

pub(super) async fn execute(
    context: &Context,
    client: &Huckleberry,
    cid: &str,
    answer: super::solids_form::SolidsSubmission,
) -> Result<()> {
    let super::solids_form::SolidsSubmission {
        at,
        foods: references,
        amount,
        reaction,
        notes,
    } = answer;
    let named = references
        .iter()
        .map(|food| food.name.clone())
        .collect::<Vec<_>>();
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
    crate::prompt::flow::runner::read(
        super::solids_form::FoodForm::create_foods(context.calendar()?, current.to_vec(), known),
        huckleberry_api::client::now_seconds(),
        context.theme,
    )
}

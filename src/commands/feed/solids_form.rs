//! Meal drafts retain their selection and each nested text editor.
use super::solids::{FoodAction, FoodSelection, match_food};
use crate::{
    cli::Reaction,
    domain::Calendar,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::models::solids::{CustomFood, FoodReference};
/// Explicit meal fields.
#[derive(Default)]
pub struct SolidsInput {
    /// Event time.
    pub at: Option<f64>,
    /// Food names supplied by flags.
    pub foods: Vec<String>,
    /// Helping description.
    pub amount: Option<String>,
    /// Reaction, when explicitly recorded.
    pub reaction: Option<Reaction>,
    /// Optional note.
    pub notes: Option<String>,
}
/// One meal ready for the API.
pub struct SolidsSubmission {
    /// Exact event time.
    pub at: f64,
    /// References matched against the prepared food catalog.
    pub foods: Vec<FoodReference>,
    /// Helping description.
    pub amount: String,
    /// Optional reaction.
    pub reaction: Option<Reaction>,
    /// Optional note.
    pub notes: Option<String>,
}
/// A selected-food list and nested custom fields form one navigable draft.
pub struct SolidsForm {
    calendar: Calendar,
    given: SolidsInput,
    at: Option<f64>,
    known: Vec<CustomFood>,
    selection: FoodSelection,
    editing_food: bool,
    food_slot: usize,
    foods_done: bool,
    custom_helping: bool,
    helping: String,
    reaction: Option<Reaction>,
    notes: Option<String>,
}
impl SolidsForm {
    /// Seeds the draft from explicit values and a food catalog read once.
    #[must_use]
    pub fn create_solids(calendar: Calendar, given: SolidsInput, known: Vec<CustomFood>) -> Self {
        Self {
            calendar,
            at: given.at,
            selection: FoodSelection {
                selected: given.foods.clone(),
            },
            helping: given.amount.clone().unwrap_or_default(),
            reaction: given.reaction,
            notes: given.notes.clone(),
            given,
            known,
            editing_food: false,
            food_slot: 0,
            foods_done: false,
            custom_helping: false,
        }
    }
    fn create_food_id(&self) -> ViewId {
        ViewId {
            field: "food-name",
            owner: Some(self.food_slot.to_string()),
        }
    }
    fn food_view(&self) -> View {
        let mut choices = Vec::new();
        if !self.selection.selected.is_empty() {
            choices.push(("Done".into(), FormValue::Choice("done".into())));
        }
        choices.extend(self.known.iter().map(|food| {
            (
                format!("Add {}", food.name),
                FormValue::Choice(format!("add:{}", food.id)),
            )
        }));
        choices.push((
            "Enter a new food name".into(),
            FormValue::Choice("new".into()),
        ));
        choices.extend(self.selection.selected.iter().map(|food| {
            (
                format!("Remove {food}"),
                FormValue::Choice(format!("remove:{food}")),
            )
        }));
        View::create_choices(
            format!("Foods: {}", self.selection.selected.join(", ")),
            choices,
            0,
        )
    }
    fn accept_food(&mut self, value: FormValue) -> Result<()> {
        let value = fields::choice(value)?;
        self.foods_done = value == "done";
        self.editing_food = value == "new";
        if let Some(id) = value.strip_prefix("add:") {
            let food = self
                .known
                .iter()
                .find(|food| food.id == id)
                .ok_or_else(|| anyhow!("food is no longer available"))?;
            self.selection.apply(FoodAction::Add(food.name.clone()));
        } else if let Some(name) = value.strip_prefix("remove:")
            && let Some(index) = self.selection.selected.iter().position(|food| food == name)
        {
            self.selection.apply(FoodAction::Remove(index));
        }
        Ok(())
    }
}
impl FormModel for SolidsForm {
    type Output = SolidsSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if self.given.at.is_none() {
            route.push(fields::create_id("at"));
        }
        if self.given.foods.is_empty() {
            route.push(fields::create_id("foods"));
        }
        if self.editing_food {
            route.push(self.create_food_id());
        }
        if self.given.amount.is_none() {
            route.push(fields::create_id("amount"));
            if self.custom_helping {
                route.push(fields::create_id("helping"));
            }
        }
        if self.given.reaction.is_none() {
            route.push(fields::create_id("reaction"));
        }
        if self.given.notes.is_none() {
            route.push(fields::create_id("notes"));
        }
        route
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        Ok(match id.field {
            "at" => fields::create_time("When?", &self.calendar, self.at, now),
            "foods" => self.food_view(),
            "food-name" => View::create_text("Food name?", "", false),
            "amount" => fields::create_choices(
                "How much?",
                &[("some", "Some"), ("custom", "Enter an amount")],
                usize::from(self.custom_helping),
            ),
            "helping" => View::create_text("How much?", &self.helping, false),
            "reaction" => fields::create_choices(
                "How did it go?",
                &[
                    ("skip", "Skip"),
                    ("loved", "Loved it"),
                    ("meh", "Took it or left it"),
                    ("hated", "Hated it"),
                    ("allergic", "Reacted badly"),
                ],
                0,
            ),
            "notes" => View::create_text(
                "Anything to note?",
                self.notes.as_deref().unwrap_or(""),
                false,
            ),
            _ => bail!("unknown meal field"),
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match id.field {
            "at" => self.at = Some(fields::instant(&value)?),
            "foods" => self.accept_food(value)?,
            "food-name" => {
                let name = fields::text(value)?;
                if name.trim().is_empty() {
                    bail!("enter a food name");
                }
                self.selection.apply(FoodAction::Add(name));
                self.food_slot += 1;
                self.editing_food = false;
            }
            "amount" => self.custom_helping = fields::choice(value)? == "custom",
            "helping" => {
                let helping = fields::text(value)?;
                if helping.trim().is_empty() {
                    bail!("enter an amount");
                }
                self.helping = helping;
            }
            "reaction" => {
                self.reaction = if matches!(&value,FormValue::Choice(value) if value=="skip") {
                    None
                } else {
                    Some(fields::parse_choice(value)?)
                }
            }
            "notes" => self.notes = fields::optional_text(value)?,
            _ => bail!("unknown meal field"),
        }
        Ok(())
    }
    fn forward(&self, id: &ViewId) -> Option<ViewId> {
        if id.field == "food-name" || id.field == "foods" && !self.foods_done && !self.editing_food
        {
            return Some(fields::create_id("foods"));
        }
        let route = self.route();
        route
            .iter()
            .position(|view| view == id)
            .and_then(|index| route.get(index + 1))
            .cloned()
    }
    fn complete(&self) -> Result<SolidsSubmission> {
        if self.selection.selected.is_empty() {
            bail!("no foods: pass --food <NAME>");
        }
        let amount = if self.custom_helping || self.given.amount.is_some() {
            self.helping.clone()
        } else {
            "some".into()
        };
        if amount.trim().is_empty() {
            bail!("enter an amount");
        }
        Ok(SolidsSubmission {
            at: self.at.ok_or_else(|| anyhow!("choose a time"))?,
            foods: self
                .selection
                .selected
                .iter()
                .map(|food| match_food(food, &self.known, &amount))
                .collect(),
            amount,
            reaction: self.reaction,
            notes: self.notes.clone(),
        })
    }
}

/// The same food list used on its own inside an entry's field editor.
pub struct FoodForm(SolidsForm);
impl FoodForm {
    /// Opens the shared food controls with the entry's existing selection.
    #[must_use]
    pub fn create_foods(calendar: Calendar, selected: Vec<String>, known: Vec<CustomFood>) -> Self {
        let mut meal = SolidsForm::create_solids(calendar, SolidsInput::default(), known);
        meal.selection.selected = selected;
        Self(meal)
    }
}
impl FormModel for FoodForm {
    type Output = Vec<String>;
    fn route(&self) -> Vec<ViewId> {
        self.0
            .route()
            .into_iter()
            .filter(|id| matches!(id.field, "foods" | "food-name"))
            .collect()
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        self.0.create_view(id, now)
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()> {
        self.0.accept(id, value, now)
    }
    fn forward(&self, id: &ViewId) -> Option<ViewId> {
        self.0
            .forward(id)
            .filter(|id| matches!(id.field, "foods" | "food-name"))
    }
    fn complete(&self) -> Result<Vec<String>> {
        if self.0.selection.selected.is_empty() {
            bail!("choose at least one food");
        }
        Ok(self.0.selection.selected.clone())
    }
}

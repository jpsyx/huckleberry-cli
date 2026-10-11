//! Food fields embed the meal's list and custom-name controls with entry-owned IDs.
use super::{EditForm, entry::EntryDraft};
use crate::{
    commands::feed::solids_form::FoodForm,
    domain::Calendar,
    edit::Draft,
    prompt::flow::{FormModel, FormValue, View, ViewId},
};
use anyhow::{Result, anyhow};
use huckleberry_api::models::solids::CustomFood;
impl EditForm {
    /// Attaches a catalog read once during preparation to each entry's food editor.
    #[must_use]
    pub fn with_foods(mut self, known: &[CustomFood]) -> Self {
        for draft in self.drafts.values_mut() {
            draft.create_foods(&self.calendar, known);
        }
        self
    }
}
impl EntryDraft {
    pub(super) fn create_foods(&mut self, calendar: &Calendar, known: &[CustomFood]) {
        if let Some(Draft::Solids(meal)) = self.shown() {
            self.foods = Some(FoodForm::create_foods(
                calendar.clone(),
                meal.foods,
                known.to_vec(),
            ));
        }
    }
    pub(super) fn food_route(&self) -> Vec<ViewId> {
        self.foods.as_ref().map_or_else(Vec::new, |foods| {
            foods
                .route()
                .into_iter()
                .map(|id| self.scope_food(id))
                .collect()
        })
    }
    fn scope_food(&self, id: ViewId) -> ViewId {
        ViewId {
            field: id.field,
            owner: Some(format!(
                "{}/foods/{}",
                self.target.token,
                id.owner.unwrap_or_default()
            )),
        }
    }
    pub(super) fn food_view(&self, id: &ViewId, now: f64) -> Result<View> {
        self.foods
            .as_ref()
            .ok_or_else(|| anyhow!("choose foods"))?
            .create_view(&unscoped(id), now)
    }
    pub(super) fn accept_food(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()> {
        let foods = self.foods.as_mut().ok_or_else(|| anyhow!("choose foods"))?;
        let id = unscoped(id);
        foods.accept(&id, value, now)?;
        if foods.forward(&id).is_none() {
            let value = foods.complete()?.join(", ");
            self.record("foods", &value)?;
        }
        Ok(())
    }
    pub(super) fn food_forward(&self, id: &ViewId) -> Option<ViewId> {
        self.foods
            .as_ref()?
            .forward(&unscoped(id))
            .map(|id| self.scope_food(id))
    }
}
fn unscoped(id: &ViewId) -> ViewId {
    ViewId {
        field: id.field,
        owner: if id.field == "food-name" {
            id.owner
                .as_ref()
                .and_then(|owner| owner.rsplit('/').next())
                .map(str::to_owned)
        } else {
            None
        },
    }
}

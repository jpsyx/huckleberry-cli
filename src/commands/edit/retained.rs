//! Each selected entry owns its field draft; navigation never saves it.
mod entry;
mod foods;
mod questions;
use self::entry::EntryDraft;
use crate::{
    domain::Calendar,
    edit::Draft,
    listing::Row,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields, listing::ListingControl},
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::models::SleepTimer;
use std::collections::BTreeMap;
/// Original entry state, including live timer identity when applicable.
#[derive(Clone)]
pub struct EditTarget {
    /// Stable selector, not a row index.
    pub token: String,
    /// Exact stored event time.
    pub started: f64,
    /// Editable details when supported.
    pub draft: Option<Draft>,
    /// Original live sleep, whose UUID must match at save time.
    pub timer: Option<SleepTimer>,
}
/// Collected changes for exactly one original target.
pub struct EditSubmission {
    /// Identity and original data being edited.
    pub target: EditTarget,
    /// Explicit detail changes and any supplied time flag.
    pub changes: Vec<String>,
    /// A time corrected through the form, without reparsing or rounding.
    pub started: Option<f64>,
}
/// Picker, field menu and nested field controls share one retained state.
pub struct EditForm {
    calendar: Calendar,
    rows: Vec<Row>,
    drafts: BTreeMap<String, EntryDraft>,
    selected: Option<String>,
    fixed: bool,
    explicit: Vec<String>,
}
impl EditForm {
    /// Builds owned drafts for the prepared history and live entries.
    pub fn create_edit(
        calendar: Calendar,
        rows: Vec<Row>,
        targets: Vec<EditTarget>,
        selected: Option<String>,
        explicit: Vec<String>,
    ) -> Result<Self> {
        let drafts = targets
            .into_iter()
            .map(|target| (target.token.clone(), EntryDraft::create_draft(target)))
            .collect();
        let form = Self {
            calendar,
            rows,
            drafts,
            fixed: selected.is_some(),
            selected,
            explicit,
        };
        if form.fixed {
            form.current()?;
        }
        Ok(form.with_foods(&[]))
    }
    /// Restores pending options into the selected entry without suppressing its field menu.
    pub(crate) fn with_changes(mut self, changes: Vec<String>, started: f64) -> Self {
        if let Some(draft) = self
            .selected
            .as_ref()
            .and_then(|token| self.drafts.get_mut(token))
        {
            draft.changes = changes;
            draft.started = started;
        }
        self
    }
    fn current(&self) -> Result<&EntryDraft> {
        self.selected
            .as_ref()
            .and_then(|token| self.drafts.get(token))
            .ok_or_else(|| anyhow!("choose an entry to change"))
    }
    fn create_id(&self, field: &'static str) -> ViewId {
        let owner = self.selected.as_ref().map(|token| {
            if matches!(field, "field" | "input") {
                format!(
                    "{token}/{}",
                    self.current()
                        .ok()
                        .and_then(|draft| draft.field)
                        .unwrap_or("")
                )
            } else {
                token.clone()
            }
        });
        ViewId { field, owner }
    }
}
impl FormModel for EditForm {
    type Output = EditSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if !self.fixed {
            route.push(fields::create_id("entry"));
        }
        if self.explicit.is_empty() {
            route.push(self.create_id("fields"));
            if let Ok(draft) = self.current()
                && draft.field.is_some()
            {
                route.push(self.create_id("field"));
                if draft.nested {
                    if draft.field == Some("foods") {
                        route.extend(draft.food_route());
                    } else {
                        route.push(self.create_id("input"));
                    }
                }
            }
        }
        route
    }
    fn create_view(&self, id: &ViewId, now: f64) -> Result<View> {
        match id.field {
            "entry" => Ok(View::create_listing(ListingControl::create_listing(
                "Change which entry?",
                &crate::render::log::COLUMNS,
                self.rows.clone(),
                self.selected.as_deref(),
            ))),
            "fields" => Ok(self.current()?.menu(&self.calendar)),
            "field" => self.current()?.question(&self.calendar, now),
            "input" => self.current()?.input(),
            "foods" | "food-name" => self.current()?.food_view(id, now),
            _ => bail!("unknown edit view"),
        }
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, now: f64) -> Result<()> {
        if id.field == "entry" {
            let FormValue::Picked(token) = value else {
                bail!("choose an entry");
            };
            if !self.drafts.contains_key(&token) {
                bail!("that entry is no longer available");
            }
            self.selected = Some(token);
            return Ok(());
        }
        let token = self
            .selected
            .as_ref()
            .ok_or_else(|| anyhow!("choose an entry"))?;
        let draft = self.drafts.get_mut(token).expect("selected entry exists");
        if matches!(id.field, "foods" | "food-name") {
            draft.accept_food(id, value, now)
        } else {
            draft.accept(id.field, value, &self.calendar, now)
        }
    }
    fn forward(&self, id: &ViewId) -> Option<ViewId> {
        if id.field == "entry" {
            return self.explicit.is_empty().then(|| self.create_id("fields"));
        }
        let draft = self.current().ok()?;
        match id.field {
            "fields" => draft.field.map(|_| self.create_id("field")),
            "field" if draft.nested && draft.field == Some("foods") => {
                draft.food_route().into_iter().next()
            }
            "field" if draft.nested => Some(self.create_id("input")),
            "foods" | "food-name" if draft.nested => draft.food_forward(id),
            "foods" | "food-name" | "field" | "input" => Some(self.create_id("fields")),
            _ => None,
        }
    }
    fn complete(&self) -> Result<EditSubmission> {
        let draft = self.current()?;
        draft.validate()?;
        Ok(EditSubmission {
            target: draft.target.clone(),
            changes: if self.explicit.is_empty() {
                draft.changes.clone()
            } else {
                self.explicit.clone()
            },
            started: draft.corrected_start(),
        })
    }
}

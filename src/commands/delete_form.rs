//! A delete confirmation belongs to the exact selected row.
use crate::{
    edit,
    listing::{Column, Row},
    prompt::{
        self,
        flow::{FormModel, FormValue, View, ViewId, fields, listing::ListingControl},
    },
    theme::Tone,
};
use anyhow::{Result, anyhow, bail};
use huckleberry_api::RowRef;
/// An original row identity and its description.
#[derive(Clone)]
pub struct DeleteTarget {
    /// Full stored location, including batch key.
    pub at: RowRef,
    /// Original start, used for summary repair.
    pub started: f64,
    /// Description presented before confirmation.
    pub description: String,
}
/// A deletion authorized for one target only.
pub type DeleteSubmission = DeleteTarget;
/// Collection performs no writes, and changing target clears accepted confirmation.
pub struct DeleteForm {
    rows: Vec<Row>,
    columns: &'static [Column],
    targets: Vec<DeleteTarget>,
    selected: Option<String>,
    fixed: bool,
    yes: bool,
    confirmed: Option<String>,
}
impl DeleteForm {
    /// Creates a picker and confirmation, omitting only explicitly supplied answers.
    #[must_use]
    pub const fn create_delete(
        rows: Vec<Row>,
        columns: &'static [Column],
        targets: Vec<DeleteTarget>,
        selected: Option<String>,
        yes: bool,
    ) -> Self {
        Self {
            rows,
            columns,
            targets,
            fixed: selected.is_some(),
            selected,
            yes,
            confirmed: None,
        }
    }
    fn target(&self) -> Result<&DeleteTarget> {
        self.targets
            .iter()
            .find(|target| Some(edit::token_for(&target.at)).as_ref() == self.selected.as_ref())
            .ok_or_else(|| anyhow!("choose an entry to delete"))
    }
}
impl FormModel for DeleteForm {
    type Output = DeleteSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if !self.fixed {
            route.push(fields::create_id("entry"));
        }
        if !self.yes {
            route.push(ViewId {
                field: "confirm",
                owner: self.selected.clone(),
            });
        }
        route
    }
    fn create_view(&self, id: &ViewId, _: f64) -> Result<View> {
        Ok(match id.field {
            "entry" => View::create_listing(ListingControl::create_listing(
                "Delete which entry?",
                self.columns,
                self.rows.clone(),
                self.selected.as_deref(),
            )),
            "confirm" => fields::create_choices(
                "This cannot be undone. Delete it?",
                &[("no", "No"), ("yes", "Yes")],
                0,
            )
            .with_description(
                format!("About to delete {}.", self.target()?.description),
                Tone::Attention,
            ),
            _ => bail!("unknown deletion field"),
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        match (id.field, value) {
            ("entry", FormValue::Picked(token)) => {
                if self.selected.as_ref() != Some(&token) {
                    self.confirmed = None;
                }
                self.selected = Some(token);
                self.target()?;
            }
            ("confirm", FormValue::Choice(answer)) => {
                if answer != "yes" {
                    return Err(prompt::Cancelled.into());
                }
                self.confirmed = self.selected.clone();
            }
            _ => bail!("choose an entry or confirm it"),
        }
        Ok(())
    }
    fn complete(&self) -> Result<DeleteSubmission> {
        if !self.yes && (self.selected.is_none() || self.confirmed != self.selected) {
            bail!("confirm the selected entry with --yes");
        }
        self.target().cloned()
    }
}

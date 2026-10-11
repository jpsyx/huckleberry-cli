//! Child choice and the optional remember question form one reversible draft.
use crate::prompt::flow::{FormModel, FormValue, View, ViewId, fields};
use anyhow::{Result, anyhow, bail};
/// A selected child and the explicit persistence decision.
pub struct ChildSubmission {
    /// Stable child ID.
    pub cid: String,
    /// Display name.
    pub name: String,
    /// Whether to save this choice.
    pub remember: bool,
}
/// Owned account rows avoid a second account read while navigating.
pub struct ChildForm {
    children: Vec<(String, String)>,
    given: bool,
    selected: Option<String>,
    ask_remember: bool,
    remember: bool,
}
impl ChildForm {
    /// Creates either an explicit child-use action or a prerequisite choice.
    pub fn create_child(
        children: Vec<(String, String)>,
        cid: Option<String>,
        ask_remember: bool,
    ) -> Result<Self> {
        if children.is_empty() {
            bail!("this account has no children on it");
        }
        if let Some(cid) = &cid
            && !children.iter().any(|(id, _)| id == cid)
        {
            bail!(super::super::child::unknown_child_message(cid, &children));
        }
        Ok(Self {
            children,
            given: cid.is_some(),
            selected: cid,
            ask_remember,
            remember: !ask_remember,
        })
    }
}
impl FormModel for ChildForm {
    type Output = ChildSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if !self.given {
            route.push(fields::create_id("child"));
        }
        if self.ask_remember {
            route.push(fields::create_id("remember"));
        }
        route
    }
    fn create_view(&self, id: &ViewId, _: f64) -> Result<View> {
        Ok(if id.field == "remember" {
            fields::create_choices("Remember this child?", &[("yes", "Yes"), ("no", "No")], 0)
        } else {
            View::create_choices(
                "Which child?",
                self.children
                    .iter()
                    .map(|(cid, name)| (name.clone(), FormValue::Choice(cid.clone())))
                    .collect(),
                self.children
                    .iter()
                    .position(|(cid, _)| Some(cid) == self.selected.as_ref())
                    .unwrap_or_default(),
            )
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        let choice = fields::choice(value)?;
        if id.field == "remember" {
            self.remember = choice == "yes";
        } else {
            if !self.children.iter().any(|(cid, _)| *cid == choice) {
                bail!("choose a child");
            }
            self.selected = Some(choice);
        }
        Ok(())
    }
    fn complete(&self) -> Result<ChildSubmission> {
        let cid = self
            .selected
            .clone()
            .ok_or_else(|| anyhow!("provide --child <CID> (or `child use`)"))?;
        let name = self
            .children
            .iter()
            .find(|(id, _)| *id == cid)
            .map_or_else(|| cid.clone(), |(_, name)| name.clone());
        Ok(ChildSubmission {
            cid,
            name,
            remember: self.remember,
        })
    }
}

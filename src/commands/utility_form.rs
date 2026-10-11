//! Retained utility drafts shared by the shell and standalone commands.
mod child;
mod login;
mod settings;
pub use child::{ChildForm, ChildSubmission};
pub use login::{LoginForm, LoginInput, LoginSubmission};
pub use settings::{SettingsForm, SettingsSubmission};

use crate::prompt::flow::{FormModel, FormValue, View, ViewId, fields};
use anyhow::{Result, bail};

/// A single required food name, retained until it is submitted.
pub struct FoodForm {
    given: bool,
    name: Option<String>,
}
impl FoodForm {
    /// An explicit name suppresses collection.
    #[must_use]
    pub const fn create_food(name: Option<String>) -> Self {
        Self {
            given: name.is_some(),
            name,
        }
    }
}
impl FormModel for FoodForm {
    type Output = String;
    fn route(&self) -> Vec<ViewId> {
        if self.given {
            Vec::new()
        } else {
            vec![fields::create_id("name")]
        }
    }
    fn create_view(&self, _: &ViewId, _: f64) -> Result<View> {
        Ok(View::create_text(
            "What is it called?",
            self.name.as_deref().unwrap_or_default(),
            false,
        ))
    }
    fn accept(&mut self, _: &ViewId, value: FormValue, _: f64) -> Result<()> {
        self.name = Some(fields::text(value)?);
        self.complete().map(|_| ())
    }
    fn complete(&self) -> Result<String> {
        match &self.name {
            Some(name) if !name.trim().is_empty() => Ok(name.clone()),
            _ => bail!("provide a food name with <NAME>"),
        }
    }
}

/// A command with no questions still passes through consuming execution.
pub struct ReadyForm;
impl FormModel for ReadyForm {
    type Output = ();
    fn route(&self) -> Vec<ViewId> {
        Vec::new()
    }
    fn create_view(&self, _: &ViewId, _: f64) -> Result<View> {
        bail!("no questions")
    }
    fn accept(&mut self, _: &ViewId, _: FormValue, _: f64) -> Result<()> {
        bail!("no questions")
    }
    fn complete(&self) -> Result<()> {
        Ok(())
    }
}

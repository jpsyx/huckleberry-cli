//! First-use settings are an in-memory draft until the final answer is accepted.
use crate::{
    config::Config,
    domain::today::DayMode,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
};
use anyhow::Result;
/// One collection over the settings missing when setup began.
pub struct SetupForm {
    draft: Config,
    fields: Vec<&'static str>,
    suggested: DayMode,
}
impl SetupForm {
    /// Uses existing age-based defaults without writing any settings.
    #[must_use]
    pub fn create_setup(config: Config, age: Option<i64>) -> Self {
        Self {
            fields: super::missing(&config),
            draft: config,
            suggested: super::default_mode(age),
        }
    }
}
impl FormModel for SetupForm {
    type Output = Config;
    fn route(&self) -> Vec<ViewId> {
        self.fields
            .iter()
            .map(|field| fields::create_id(field))
            .collect()
    }
    fn create_view(&self, id: &ViewId, _: f64) -> Result<View> {
        if id.field == "day_mode" {
            return Ok(fields::create_choices(
                "How should \"today\" be counted? It decides what `fed today` covers.",
                &[
                    (DayMode::Continuous.key(), DayMode::Continuous.describe()),
                    (DayMode::Discrete.key(), DayMode::Discrete.describe()),
                ],
                usize::from(self.suggested == DayMode::Discrete),
            ));
        }
        let (hours, fallback) = super::choices_for(id.field);
        let current = self
            .draft
            .get(id.field)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback.into());
        let label = if id.field == "day_start" {
            "What time does a day start? Anything earlier counts as the day before."
        } else {
            "What time does a day end? Night runs from here to the next day's start."
        };
        Ok(View::create_choices(
            label,
            hours
                .iter()
                .map(|hour| {
                    (
                        super::run::label_for(hour),
                        FormValue::Choice((*hour).into()),
                    )
                })
                .collect(),
            hours
                .iter()
                .position(|hour| *hour == current)
                .unwrap_or_default(),
        ))
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        self.draft.set(id.field, &fields::choice(value)?)
    }
    fn complete(&self) -> Result<Config> {
        let missing = super::missing(&self.draft);
        if !missing.is_empty() {
            super::refuse(&missing)?;
        }
        Ok(self.draft.clone())
    }
}

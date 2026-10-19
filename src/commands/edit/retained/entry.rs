//! Pending values reuse edit's existing explicit-change construction.
use super::EditTarget;
use crate::{
    commands::edit::{form, form_pump},
    domain::Calendar,
    edit::Draft,
    interactive::options::fields,
    prompt::flow::{FormValue, View},
};
use anyhow::{Result, anyhow, bail};

pub(super) struct EntryDraft {
    pub target: EditTarget,
    pub resumable: bool,
    pub changes: Vec<String>,
    pub started: f64,
    pub stopped: Option<f64>,
    pub field: Option<&'static str>,
    pub nested: bool,
    pub foods: Option<crate::commands::feed::solids_form::FoodForm>,
}
impl EntryDraft {
    pub const fn create_draft(target: EditTarget) -> Self {
        Self {
            started: target.started,
            target,
            resumable: false,
            changes: vec![],
            stopped: None,
            field: None,
            nested: false,
            foods: None,
        }
    }
    pub fn shown(&self) -> Option<Draft> {
        form::shown_draft(self.target.draft.as_ref(), &self.changes)
    }
    pub fn menu(&self, calendar: &Calendar) -> View {
        let names = form::visible_fields(
            self.shown().as_ref(),
            self.target.token == super::super::live::TOKEN,
        );
        let items = form::menu_items(
            self.target.draft.as_ref(),
            &names,
            &self.changes,
            self.started,
            calendar,
        );
        let values = std::iter::once("done")
            .chain(names)
            .map(|field| FormValue::Choice(field.into()));
        let mut choices: Vec<_> = items
            .into_iter()
            .zip(values)
            .map(|(item, value)| {
                (
                    format!("{} · {}", item.label, item.detail.unwrap_or_default()),
                    value,
                )
            })
            .collect();
        if self.can_resume() {
            choices.push((
                "Resume sleep · continue from its original start".into(),
                FormValue::Choice("resume".into()),
            ));
        }
        View::create_choices("Fields to change", choices, 0)
    }
    pub fn accept(
        &mut self,
        view: &str,
        value: FormValue,
        calendar: &Calendar,
        _now: f64,
    ) -> Result<()> {
        if view == "fields" {
            return self.select_field(value);
        }
        let field = self.field.ok_or_else(|| anyhow!("choose a field"))?;
        if let FormValue::Instant(at) = value {
            return self.record_time(field, at, calendar);
        }
        if let FormValue::Volume { units, amount } = value {
            return self.record_volume(field, units, amount);
        }
        if view == "field" {
            return self.accept_choice(field, value);
        }
        self.record(field, &crate::prompt::flow::fields::text(value)?)
    }
    /// Menu navigation changes a draft; only the completed form can execute it.
    fn select_field(&mut self, value: FormValue) -> Result<()> {
        let selected = crate::prompt::flow::fields::choice(value)?;
        if selected == "resume" {
            if !self.can_resume() {
                bail!("save other edits before resuming the latest sleep");
            }
            self.changes = vec!["resume=true".into()];
            self.field = None;
            self.nested = false;
            return Ok(());
        }
        self.field = if selected == "done" {
            None
        } else {
            Some(
                form::visible_fields(
                    self.shown().as_ref(),
                    self.target.token == super::super::live::TOKEN,
                )
                .into_iter()
                .find(|field| *field == selected)
                .ok_or_else(|| anyhow!("choose a field"))?,
            )
        };
        self.nested = false;
        Ok(())
    }
    fn accept_choice(&mut self, field: &str, value: FormValue) -> Result<()> {
        let answer = crate::prompt::flow::fields::choice(value)?;
        self.nested = answer == "change";
        match answer.as_str() {
            "change" => Ok(()),
            "keep" => {
                let shown = self.shown();
                form_pump::keep_field(
                    &mut self.changes,
                    self.target.draft.as_ref(),
                    shown.as_ref(),
                    field,
                )?;
                Ok(())
            }
            "clear" => self.record(field, ""),
            value => self.record(field, value.strip_prefix("value:").unwrap_or(value)),
        }
    }
    pub(super) fn record(&mut self, field: &str, value: &str) -> Result<()> {
        let shown = self.shown();
        form::record_value(&mut self.changes, shown.as_ref(), field, value)?;
        self.nested = false;
        Ok(())
    }
    fn record_volume(&mut self, field: &str, units: crate::cli::Units, amount: f64) -> Result<()> {
        let shown = self.shown();
        if let Some(Draft::Bottle(bottle)) = &shown
            && bottle.units == units
            && bottle.amount.to_bits() == amount.to_bits()
        {
            self.nested = false;
            return Ok(());
        }
        if let Some(Draft::Pump(pump)) = self.shown() {
            form_pump::record_volume(&mut self.changes, &pump, field, amount, units)?;
        } else {
            fields::update_change(&mut self.changes, "amount", Some(&amount.to_string()));
            fields::update_change(&mut self.changes, "units", Some(units.as_str()));
        }
        self.nested = false;
        Ok(())
    }
    fn record_time(&mut self, field: &str, at: f64, _calendar: &Calendar) -> Result<()> {
        if field == "duration" {
            if at <= self.started {
                bail!("choose a time after it began");
            }
            self.stopped = Some(at);
            form::set_stop(
                &mut self.changes,
                self.target.draft.as_ref(),
                self.started,
                at,
            );
        } else {
            self.started = at;
            if let Some(stop) = self.stopped
                && stop > at
            {
                form::set_stop(&mut self.changes, self.target.draft.as_ref(), at, stop);
            }
        }
        self.nested = false;
        Ok(())
    }
    fn can_resume(&self) -> bool {
        self.resumable && self.changes.is_empty() && self.corrected_start().is_none()
    }
    pub fn corrected_start(&self) -> Option<f64> {
        (self.started.to_bits() != self.target.started.to_bits()).then_some(self.started)
    }
    pub fn validate(&self) -> Result<()> {
        if self.stopped.is_some_and(|stop| stop <= self.started) {
            bail!("That start is after the stop you chose: choose Edit stop again.");
        }
        if let Some(mut draft) = self.target.draft.clone() {
            draft.apply(&self.changes)?;
        }
        Ok(())
    }
}

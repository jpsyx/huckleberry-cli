//! Each setting owns its value view, so changing keys cannot transfer a buffer.
use crate::{
    config::Config,
    prompt::flow::{FormModel, FormValue, View, ViewId, fields},
};
use anyhow::{Result, anyhow, bail};
use std::collections::BTreeMap;
/// One validated setting, saved only after collection.
pub struct SettingsSubmission {
    /// Setting name.
    pub key: String,
    /// New value.
    pub value: String,
}
/// Retains a separate draft value for each visited key.
pub struct SettingsForm {
    config: Config,
    given_key: Option<String>,
    given_value: Option<String>,
    key: Option<String>,
    values: BTreeMap<String, String>,
}
impl SettingsForm {
    /// Validates explicit keys and values before any file can be written.
    pub fn create_settings(
        config: Config,
        key: Option<String>,
        value: Option<String>,
    ) -> Result<Self> {
        if let Some(key) = &key {
            if !Config::KEYS.contains(&key.as_str()) {
                bail!("unknown setting `{key}`");
            }
            if let Some(value) = &value {
                config.clone().set(key, value)?;
            }
        }
        Ok(Self {
            config,
            given_key: key.clone(),
            key,
            given_value: value,
            values: BTreeMap::new(),
        })
    }
}
impl FormModel for SettingsForm {
    type Output = SettingsSubmission;
    fn route(&self) -> Vec<ViewId> {
        let mut route = Vec::new();
        if self.given_key.is_none() {
            route.push(fields::create_id("key"));
        }
        if self.given_value.is_none()
            && let Some(key) = &self.key
        {
            route.push(ViewId {
                field: "value",
                owner: Some(key.clone()),
            });
        }
        route
    }
    fn create_view(&self, id: &ViewId, _: f64) -> Result<View> {
        if id.field == "key" {
            return Ok(View::create_choices(
                "Which setting?",
                Config::KEYS
                    .iter()
                    .map(|key| {
                        (
                            format!("{key}: {}", Config::describe(key).unwrap_or_default()),
                            FormValue::Choice((*key).into()),
                        )
                    })
                    .collect(),
                Config::KEYS
                    .iter()
                    .position(|key| Some(*key) == self.key.as_deref())
                    .unwrap_or_default(),
            ));
        }
        let key = id
            .owner
            .as_deref()
            .ok_or_else(|| anyhow!("choose a setting"))?;
        let value = self
            .values
            .get(key)
            .cloned()
            .or_else(|| self.config.get(key))
            .unwrap_or_default();
        let choices = super::super::settings::value_choices(key);
        let label = format!("{}?", Config::describe(key).unwrap_or("New value"));
        Ok(if choices.is_empty() {
            View::create_text(label, &value, false)
        } else {
            View::create_choices(
                label,
                choices
                    .iter()
                    .map(|choice| (choice.value.into(), FormValue::Choice(choice.value.into())))
                    .collect(),
                choices
                    .iter()
                    .position(|choice| choice.value == value)
                    .unwrap_or_default(),
            )
        })
    }
    fn accept(&mut self, id: &ViewId, value: FormValue, _: f64) -> Result<()> {
        if id.field == "key" {
            let key = fields::choice(value)?;
            if !Config::KEYS.contains(&key.as_str()) {
                bail!("unknown setting `{key}`");
            }
            self.key = Some(key);
        } else {
            let key = id
                .owner
                .as_deref()
                .ok_or_else(|| anyhow!("choose a setting"))?;
            let (FormValue::Choice(text) | FormValue::Text(text)) = value else {
                bail!("enter a value")
            };
            self.config.clone().set(key, &text)?;
            self.values.insert(key.into(), text);
        }
        Ok(())
    }
    fn complete(&self) -> Result<SettingsSubmission> {
        let key = self.key.clone().ok_or_else(|| anyhow!("provide <KEY>"))?;
        let value = self
            .given_value
            .clone()
            .or_else(|| self.values.get(&key).cloned())
            .ok_or_else(|| anyhow!("provide <VALUE>"))?;
        self.config.clone().set(&key, &value)?;
        Ok(SettingsSubmission { key, value })
    }
}

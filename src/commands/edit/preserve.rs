//! Keeps stored enum spellings when another detail changes.
use crate::{
    domain::types::{Dataset, FeedEvent},
    edit::Draft,
};
use huckleberry_api::RowRef;
use std::collections::BTreeMap;

/// Chooses the exact stored value unless this field was deliberately changed.
#[must_use]
pub fn retained(
    original: Option<&str>,
    before: &str,
    after: &str,
    explicit: bool,
) -> Option<String> {
    if !explicit && before == after {
        original.map(str::to_owned)
    } else {
        (!after.is_empty()).then(|| after.to_owned())
    }
}

/// The original wire values plus the form's initial projection.
pub(super) struct Original {
    values: BTreeMap<String, String>,
    projected: BTreeMap<String, String>,
    explicit: Vec<String>,
}

impl Original {
    /// Captures original fields before the form can normalize them.
    pub(super) fn from_entry(
        dataset: &Dataset,
        at: &RowRef,
        before: &Draft,
        changes: &[String],
    ) -> Self {
        let mut values = BTreeMap::new();
        if let Some(diaper) = dataset
            .diapers
            .iter()
            .find(|entry| entry.at.as_ref() == Some(at))
        {
            values.insert("mode".into(), diaper.mode.clone());
            if let Some(color) = &diaper.color {
                values.insert("color".into(), color.clone());
            }
            if let Some(consistency) = &diaper.consistency {
                values.insert("consistency".into(), consistency.clone());
            }
        }
        if let Some(feed) = dataset.feeds.iter().find(|entry| entry.at() == Some(at)) {
            match feed {
                FeedEvent::Bottle {
                    bottle_type: Some(value),
                    ..
                } => {
                    values.insert("type".into(), value.clone());
                }
                FeedEvent::Solids {
                    reaction: Some(value),
                    ..
                } => {
                    values.insert("reaction".into(), value.clone());
                }
                _ => {}
            }
        }
        Self {
            values,
            projected: enum_values(before),
            explicit: changes
                .iter()
                .filter_map(|change| change.split_once('=').map(|(key, _)| key.to_owned()))
                .collect(),
        }
    }

    /// Resolves one saved enum without guessing at unfamiliar stored words.
    pub(super) fn value(&self, field: &str, after: &str) -> Option<String> {
        retained(
            self.values.get(field).map(String::as_str),
            self.projected.get(field).map_or("", String::as_str),
            after,
            self.explicit.iter().any(|key| key == field),
        )
    }
}

fn enum_values(draft: &Draft) -> BTreeMap<String, String> {
    let pairs = match draft {
        Draft::Diaper(diaper) => vec![
            ("mode", diaper.mode.to_api().as_str().to_owned()),
            (
                "color",
                diaper
                    .color
                    .map_or_else(String::new, |value| value.to_api().as_str().to_owned()),
            ),
            (
                "consistency",
                diaper
                    .consistency
                    .map_or_else(String::new, |value| value.to_api().as_str().to_owned()),
            ),
        ],
        Draft::Bottle(bottle) => vec![("type", bottle.kind.to_api().as_str().to_owned())],
        Draft::Solids(meal) => vec![(
            "reaction",
            meal.reaction
                .map_or_else(String::new, |value| value.to_api().as_str().to_owned()),
        )],
        _ => vec![],
    };
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}

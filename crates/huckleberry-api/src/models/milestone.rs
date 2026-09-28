//! `milestones/{cid}`: the firsts.
//!
//! The Python client has no methods for this collection either. It is modelled
//! because a milestone is the one row in Huckleberry that is not a
//! measurement, and a caller showing a timeline wants the name and the note,
//! not a map to pick through.

use serde::{Deserialize, Serialize};

use super::common::Number;

/// One row of `milestones/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Milestone {
    /// When it happened, in seconds.
    pub start: Number,
    /// What it was.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub name: Option<String>,
    /// The milestone's own identifier in Huckleberry's catalogue.
    #[serde(
        rename = "milestoneId",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub milestone_id: Option<String>,
    /// Which group of milestones it belongs to.
    #[serde(
        rename = "milestoneCategory",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub category: Option<String>,
    /// Where the milestone came from: the catalogue, or the parent.
    #[serde(
        rename = "milestoneSource",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub source: Option<String>,
    /// The age range the catalogue expects it in.
    #[serde(
        rename = "milestoneAgeRange",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub age_range: Option<String>,
    /// Whatever the parent typed.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub notes: Option<String>,
    /// A Firebase Storage filename.
    ///
    /// Downloading it needs an access token the document does not carry, so
    /// this is a record that a photo exists rather than a way to fetch it.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub photo: Option<String>,
    /// The timezone offset, in minutes.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::models::lenient"
    )]
    pub offset: Option<Number>,
}

impl Milestone {
    /// What to call it: the recorded name, or a stand-in.
    #[must_use]
    pub fn title(&self) -> &str {
        self.name.as_deref().unwrap_or("Milestone")
    }

    /// Whether a photo was attached.
    #[must_use]
    pub const fn has_photo(&self) -> bool {
        self.photo.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_with_a_name_uses_it() {
        let milestone: Milestone =
            serde_json::from_value(json!({ "start": 1.0, "name": "First smile" }))
                .expect("a milestone");
        assert_eq!(milestone.title(), "First smile");
        assert!(!milestone.has_photo());
    }

    #[test]
    fn a_row_without_a_name_still_has_something_to_call_it() {
        let milestone: Milestone = serde_json::from_value(json!({ "start": 1.0 })).expect("a row");
        assert_eq!(milestone.title(), "Milestone");
    }
}

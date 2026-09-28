//! `activities/{cid}`: baths, tummy time, and the rest.
//!
//! Like pump, this collection has no methods on the Python client. It is
//! modelled so that a caller reading it raw has somewhere to deserialize into.

use serde::{Deserialize, Serialize};

use super::common::Number;

string_enum! {
    /// What the activity was.
    ActivityMode {
        Bath => "bath",
        TummyTime => "tummyTime",
        StoryTime => "storyTime",
        ScreenTime => "screenTime",
        SkinToSkin => "skinToSkin",
        OutdoorPlay => "outdoorPlay",
        IndoorPlay => "indoorPlay",
        BrushTeeth => "brushTeeth",
    }
}

/// One row of `activities/{cid}/intervals`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityInterval {
    /// What it was.
    pub mode: ActivityMode,
    /// When it started, in seconds.
    pub start: Number,
    /// The timezone offset, in minutes.
    pub offset: Number,
    /// How long it lasted, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<Number>,
    /// The timezone offset at the end, in minutes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_offset: Option<Number>,
    /// When the row last changed.
    #[serde(
        rename = "lastUpdated",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub last_updated: Option<Number>,
}

/// A batch of activity rows.
pub type ActivityMultiContainer = super::common::MultiContainer<ActivityInterval>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_row_decodes_with_its_mode() {
        let row: ActivityInterval = serde_json::from_value(json!({
            "mode": "tummyTime", "start": 1.0, "offset": 0.0, "duration": 300.0,
        }))
        .expect("an activity row");
        assert_eq!(row.mode, ActivityMode::TummyTime);
    }

    #[test]
    fn an_activity_this_crate_does_not_know_is_carried_through() {
        let row: ActivityInterval =
            serde_json::from_value(json!({ "mode": "swimming", "start": 1.0, "offset": 0.0 }))
                .expect("an activity row");
        assert_eq!(row.mode.as_str(), "swimming");
    }
}

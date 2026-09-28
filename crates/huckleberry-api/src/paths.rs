//! Where each thing lives in Firestore.
//!
//! Collecting the paths here rather than spelling them at each call site is
//! worth one module on its own: health is the tracker whose history lives in
//! `data` while every other tracker uses `intervals`, and that asymmetry is
//! the kind of thing that gets copied wrong once and then read wrong for a
//! year.

/// `users/{uid}`.
#[must_use]
pub fn user(uid: &str) -> String {
    format!("users/{uid}")
}

/// `childs/{cid}`. The collection really is spelled that way.
#[must_use]
pub fn child(cid: &str) -> String {
    format!("childs/{cid}")
}

/// A tracker's root document, for example `sleep/{cid}`.
#[must_use]
pub fn tracker(name: &str, cid: &str) -> String {
    format!("{name}/{cid}")
}

/// A tracker's history subcollection, as a path.
///
/// Health's is `data`; everything else uses `intervals`.
#[must_use]
pub fn history(tracker_name: &str, cid: &str) -> String {
    format!(
        "{}/{}",
        tracker(tracker_name, cid),
        history_collection(tracker_name)
    )
}

/// The name of a tracker's history subcollection.
#[must_use]
pub fn history_collection(tracker_name: &str) -> &'static str {
    if tracker_name == HEALTH {
        "data"
    } else {
        "intervals"
    }
}

/// One row of a tracker's history.
#[must_use]
pub fn history_row(tracker_name: &str, cid: &str, row_id: &str) -> String {
    format!("{}/{row_id}", history(tracker_name, cid))
}

/// `types/{cid}`, which holds the family's own solids foods.
#[must_use]
pub fn types(cid: &str) -> String {
    format!("types/{cid}")
}

/// `types/{cid}/custom`.
#[must_use]
pub fn custom_foods(cid: &str) -> String {
    format!("{}/custom", types(cid))
}

/// `types/{cid}/custom/{food_id}`.
#[must_use]
pub fn custom_food(cid: &str, food_id: &str) -> String {
    format!("{}/{food_id}", custom_foods(cid))
}

/// The sleep tracker.
pub const SLEEP: &str = "sleep";
/// The feeding tracker, which holds nursing, bottles and solids.
pub const FEED: &str = "feed";
/// The diaper tracker, which also holds potty trips.
pub const DIAPER: &str = "diaper";
/// The health tracker: growth, medication and temperature.
pub const HEALTH: &str = "health";
/// The pumping tracker.
pub const PUMP: &str = "pump";
/// The milestones collection.
pub const MILESTONES: &str = "milestones";
/// The activities tracker.
pub const ACTIVITIES: &str = "activities";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tracker_keeps_its_history_in_intervals() {
        assert_eq!(history(SLEEP, "abc"), "sleep/abc/intervals");
        assert_eq!(history(FEED, "abc"), "feed/abc/intervals");
        assert_eq!(history(DIAPER, "abc"), "diaper/abc/intervals");
    }

    #[test]
    fn health_is_the_exception_and_keeps_its_history_in_data() {
        assert_eq!(history(HEALTH, "abc"), "health/abc/data");
        assert_eq!(history_collection(HEALTH), "data");
    }

    #[test]
    fn a_row_hangs_off_the_history_path() {
        assert_eq!(
            history_row(FEED, "abc", "1758572400000-xyz"),
            "feed/abc/intervals/1758572400000-xyz"
        );
    }

    #[test]
    fn the_child_collection_is_spelled_childs() {
        assert_eq!(child("abc"), "childs/abc");
    }

    #[test]
    fn a_custom_food_hangs_off_the_types_document() {
        assert_eq!(custom_food("abc", "food-1"), "types/abc/custom/food-1");
    }
}

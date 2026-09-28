//! The identifier shapes Huckleberry writes.
//!
//! Every one of these is a shape the app itself produces, and matching them is
//! not cosmetic. An interval id leads with the millisecond timestamp because
//! that is what makes a listing of the subcollection come out in time order
//! without an index, and a session uuid is sixteen hex characters because that
//! is what the app writes and what its own tooling expects to find.

use uuid::Uuid;

/// A session identifier: sixteen hex characters, as the app writes.
#[must_use]
pub fn session_id() -> String {
    hex(16)
}

/// A sleep interval's document id: also sixteen hex characters. Sleep is the
/// one tracker that does not use the timestamped shape.
#[must_use]
pub fn sleep_interval_id() -> String {
    hex(16)
}

/// An interval's document id: the millisecond timestamp, a hyphen, and twenty
/// hex characters. Used by feed, diaper and health.
#[must_use]
pub fn interval_id(now: f64) -> String {
    format!("{}-{}", (now * 1000.0) as i64, hex(20))
}

/// A custom food's id: a full UUID, hyphens and all, which is the one place
/// the app does not shorten it.
#[must_use]
pub fn custom_food_id() -> String {
    Uuid::new_v4().to_string()
}

/// The first `length` characters of a random UUID's hex.
fn hex(length: usize) -> String {
    let mut text = Uuid::new_v4().simple().to_string();
    text.truncate(length);
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_id_is_sixteen_hex_characters() {
        let id = session_id();
        assert_eq!(id.len(), 16);
        assert!(id.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn an_interval_id_leads_with_its_millisecond_timestamp() {
        let id = interval_id(1_758_572_400.5);
        let (timestamp, suffix) = id.split_once('-').expect("a hyphen");
        assert_eq!(timestamp, "1758572400500");
        assert_eq!(suffix.len(), 20);
    }

    #[test]
    fn interval_ids_sort_in_the_order_they_were_made() {
        let earlier = interval_id(1_758_572_400.0);
        let later = interval_id(1_758_572_401.0);
        assert!(earlier < later, "{earlier} should sort before {later}");
    }

    #[test]
    fn two_ids_made_in_the_same_millisecond_still_differ() {
        assert_ne!(interval_id(1.0), interval_id(1.0));
    }

    #[test]
    fn a_custom_food_id_keeps_its_hyphens() {
        let id = custom_food_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
    }
}

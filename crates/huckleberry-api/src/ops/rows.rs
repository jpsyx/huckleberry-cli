//! Changing a row of history that is already there.
//!
//! Every edit is a field update rather than a rewrite, for two reasons. A row
//! carries fields this crate does not model, and a rewrite would silently drop
//! them; and a row that lives inside a batch shares its document with its
//! neighbours, so the write has to name the one entry it means. Both fall out
//! of the same shape: take the paths the caller wants to change, put the row's
//! own prefix in front of each, and let Firestore's `updateMask` do the rest.

use crate::client::Huckleberry;
use crate::error::Result;
use crate::firestore::FieldUpdate;
use crate::rows::RowRef;

/// The same updates, addressed inside the row rather than at the document.
///
/// A loose row is the document, so nothing changes. A batched row is one entry
/// of `data`, so every path gains that entry's key and the mask never names a
/// bare `data`, which would delete the row's neighbours.
#[must_use]
pub fn addressed(at: &RowRef, updates: &[FieldUpdate]) -> Vec<FieldUpdate> {
    let prefix = at.field_prefix();
    if prefix.is_empty() {
        return updates.to_vec();
    }
    updates
        .iter()
        .map(|update| {
            let mut path = prefix.clone();
            path.extend(update.path.iter().cloned());
            FieldUpdate {
                path,
                value: update.value.clone(),
            }
        })
        .collect()
}

impl Huckleberry {
    /// Changes named fields of one row of history.
    ///
    /// The row has to exist: an edit of something that is not there is a
    /// mistake worth hearing about, not a new row in a place nothing will look
    /// for it.
    ///
    /// # Errors
    ///
    /// [`crate::Error::Api`] when Firestore refuses the write, which includes
    /// the row having been deleted since it was read, and
    /// [`crate::Error::Network`] when it cannot be reached.
    pub async fn update_history_row(
        &self,
        cid: &str,
        at: &RowRef,
        updates: &[FieldUpdate],
        operation: &str,
    ) -> Result<()> {
        let token = self.token().await?;
        self.firestore()
            .update(
                &token,
                &at.document_path(cid),
                &addressed(at, updates),
                operation,
            )
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_loose_rows_paths_are_left_as_they_are() {
        let updates = [FieldUpdate::set("mode", json!("pee"))];
        let addressed = addressed(&RowRef::loose("diaper", "row-one"), &updates);
        assert_eq!(addressed[0].path, vec!["mode"]);
    }

    #[test]
    fn a_batched_rows_paths_are_moved_inside_its_own_entry() {
        let updates = [
            FieldUpdate::set("mode", json!("pee")),
            FieldUpdate::delete("color"),
        ];
        let addressed = addressed(&RowRef::batched("diaper", "pack", "inner"), &updates);
        assert_eq!(addressed[0].path, vec!["data", "inner", "mode"]);
        assert_eq!(addressed[1].path, vec!["data", "inner", "color"]);
        assert_eq!(addressed[1].value, None, "a deletion stays a deletion");
    }

    #[test]
    fn a_key_with_a_dot_in_it_stays_one_segment() {
        // Split at the call site rather than here: a key the app happens to
        // have written with a dot in it must not become two fields.
        let updates = [FieldUpdate::set("mode", json!("pee"))];
        let addressed = addressed(&RowRef::batched("diaper", "pack", "a.b"), &updates);
        assert_eq!(addressed[0].path, vec!["data", "a.b", "mode"]);
    }
}

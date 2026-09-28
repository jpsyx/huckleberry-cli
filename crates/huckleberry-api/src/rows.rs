//! Where one row of history lives, and how to carry that beside the row.
//!
//! A decoded row cannot be written back. Huckleberry packs older history into
//! batch documents with the rows nested under `data`, so "this nappy" is not a
//! document id: it is a document id and, sometimes, a key inside it. Reading a
//! row and editing it are different requests, and this is what ties them
//! together.

use serde::{Deserialize, Serialize};

/// Where one history row lives in Firestore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowRef {
    /// The tracker it belongs to, as [`crate::paths`] spells it.
    pub tracker: String,
    /// The history document that holds it.
    pub document_id: String,
    /// The key it sits under, when that document is one of the packed
    /// batches. `None` is a row of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_key: Option<String>,
}

impl RowRef {
    /// A row that is a document of its own.
    #[must_use]
    pub fn loose(tracker: &str, document_id: &str) -> Self {
        Self {
            tracker: tracker.to_owned(),
            document_id: document_id.to_owned(),
            batch_key: None,
        }
    }

    /// A row nested inside a batch document.
    #[must_use]
    pub fn batched(tracker: &str, document_id: &str, key: &str) -> Self {
        Self {
            tracker: tracker.to_owned(),
            document_id: document_id.to_owned(),
            batch_key: Some(key.to_owned()),
        }
    }

    /// Whether this row shares its document with others.
    #[must_use]
    pub const fn is_batched(&self) -> bool {
        self.batch_key.is_some()
    }

    /// The path of the document to write, which for a batched row is the
    /// batch rather than the row.
    #[must_use]
    pub fn document_path(&self, cid: &str) -> String {
        crate::paths::history_row(&self.tracker, cid, &self.document_id)
    }

    /// The prefix every field path of an edit hangs off: nothing for a loose
    /// row, and the entry's own key for a batched one.
    #[must_use]
    pub fn field_prefix(&self) -> Vec<String> {
        self.batch_key
            .as_ref()
            .map_or_else(Vec::new, |key| vec!["data".to_owned(), key.clone()])
    }
}

/// A row, and where it came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Located<T> {
    /// Where the row lives.
    pub at: RowRef,
    /// The row itself.
    pub row: T,
}

impl<T> Located<T> {
    /// A row at a place.
    pub const fn new(at: RowRef, row: T) -> Self {
        Self { at, row }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_loose_row_is_written_at_its_own_path_with_no_prefix() {
        let at = RowRef::loose("diaper", "row-one");
        assert_eq!(at.document_path("c1"), "diaper/c1/intervals/row-one");
        assert!(at.field_prefix().is_empty());
        assert!(!at.is_batched());
    }

    #[test]
    fn a_batched_row_is_written_at_the_batch_under_its_own_key() {
        let at = RowRef::batched("feed", "pack-one", "inner");
        assert_eq!(at.document_path("c1"), "feed/c1/intervals/pack-one");
        assert_eq!(at.field_prefix(), vec!["data", "inner"]);
        assert!(at.is_batched());
    }

    #[test]
    fn health_keeps_its_history_somewhere_else_and_a_reference_knows_that() {
        let at = RowRef::loose("health", "row-one");
        assert_eq!(at.document_path("c1"), "health/c1/data/row-one");
    }
}

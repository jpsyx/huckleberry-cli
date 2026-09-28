//! What a listing is made of, and what a search does to it.
//!
//! Every list this tool shows is the same shape: titled, column-aligned rows,
//! sometimes grouped under headings, always worth searching. A command hands
//! this module [`Row`]s and never works out a column width, a colour or a
//! scroll offset for itself.
//!
//! Ported from the listing view in this author's `jpsyx` CLI, which arrived at
//! this shape over a dozen commands: the interesting part is that everything
//! except drawing and reading keys is pure, so the layout, the filtering and
//! the movement rules are tested without a terminal.

use crate::theme::Tone;

/// What a cell is for, which is what decides how it is painted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The thing being listed: a time, a name, an id.
    Key,
    /// What kind of thing it is, painted in the row's own colour.
    Kind,
    /// The answer the reader came for.
    Value,
    /// Secondary: a hint, a unit, a note.
    Muted,
}

impl Role {
    /// The colour a cell in this role is painted, given the row's own.
    #[must_use]
    pub const fn tone(self, row: Option<Tone>) -> Tone {
        match self {
            Self::Kind => match row {
                Some(tone) => tone,
                None => Tone::Accent,
            },
            Self::Value => Tone::Value,
            // The same grey, and not the same thing: a key is what the reader
            // scans down (a time, a name) and reads as structure, while muted
            // is text they may skip. They are one colour today and the roles
            // are what a change would be made in terms of.
            Self::Key | Self::Muted => Tone::Muted,
        }
    }
}

/// One column of a listing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    /// The heading, which is also the name a search field goes by.
    pub header: &'static str,
    /// What the cells under it are for.
    pub role: Role,
}

impl Column {
    /// A column with a heading and a role.
    #[must_use]
    pub const fn new(header: &'static str, role: Role) -> Self {
        Self { header, role }
    }
}

/// One listed record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Row {
    /// What the caller knows this row by, handed back when it is chosen.
    pub key: String,
    /// The group this row sits under. Empty means ungrouped.
    pub group: String,
    /// One cell per column.
    pub cells: Vec<String>,
    /// The row's own colour, for the cell in the [`Role::Kind`] column.
    pub tone: Option<Tone>,
    /// A second line under the row: a note somebody typed.
    pub note: Option<String>,
    /// Whether this row can be chosen. A row that cannot is still listed, and
    /// still says why when somebody tries.
    pub selectable: bool,
    /// Why it cannot be chosen, when it cannot.
    pub refusal: Option<String>,
    /// What to show when it is chosen and the caller only wants to look.
    pub detail: Vec<(String, String)>,
}

impl Row {
    /// A row with a key and its cells, selectable and ungrouped.
    #[must_use]
    pub fn new(key: impl Into<String>, cells: impl IntoIterator<Item = String>) -> Self {
        Self {
            key: key.into(),
            group: String::new(),
            cells: cells.into_iter().collect(),
            tone: None,
            note: None,
            selectable: true,
            refusal: None,
            detail: Vec::new(),
        }
    }

    /// Puts the row under a heading.
    #[must_use]
    pub fn group(mut self, key: impl Into<String>) -> Self {
        self.group = key.into();
        self
    }

    /// Paints the row's kind cell in its own colour.
    #[must_use]
    pub const fn tone(mut self, tone: Tone) -> Self {
        self.tone = Some(tone);
        self
    }

    /// Adds the second line under the row.
    #[must_use]
    pub fn note(mut self, note: Option<String>) -> Self {
        self.note = note;
        self
    }

    /// Says this row cannot be chosen, and why.
    #[must_use]
    pub fn refused(mut self, why: impl Into<String>) -> Self {
        self.selectable = false;
        self.refusal = Some(why.into());
        self
    }

    /// What to show about this row when it is looked at rather than acted on.
    #[must_use]
    pub fn detail(mut self, pairs: Vec<(String, String)>) -> Self {
        self.detail = pairs;
        self
    }
}

/// Whether a row matches a query.
///
/// Case-insensitive, and every whitespace-separated term has to appear
/// somewhere in the row, so `diaper big` narrows rather than widens. An empty
/// query matches everything.
#[must_use]
pub fn matches(row: &Row, query: &str) -> bool {
    let haystack = searchable(row);
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

/// Everything about a row a search can see: its cells, its group and its note.
fn searchable(row: &Row) -> String {
    let mut text = row.group.to_lowercase();
    for cell in &row.cells {
        text.push('\n');
        text.push_str(&cell.to_lowercase());
    }
    if let Some(note) = &row.note {
        text.push('\n');
        text.push_str(&note.to_lowercase());
    }
    text
}

/// The rows a query leaves.
#[must_use]
pub fn filter<'a>(rows: &'a [Row], query: &str) -> Vec<&'a Row> {
    rows.iter().filter(|row| matches(row, query)).collect()
}

/// One heading and the rows under it, each with its place in the filtered list.
#[derive(Debug)]
pub struct Group<'a> {
    /// The group's key, which the heading is rendered from.
    pub key: String,
    /// The rows, and where each sits in the filtered list.
    pub rows: Vec<(usize, &'a Row)>,
}

/// Splits filtered rows into groups, in the order they first appear.
///
/// A group keeps its heading as long as one of its rows survives the search,
/// even when the heading itself does not match: the heading is context for the
/// rows, not one of them.
#[must_use]
pub fn grouped<'a>(rows: &[&'a Row]) -> Vec<Group<'a>> {
    let mut groups: Vec<Group<'a>> = Vec::new();
    for (position, row) in rows.iter().enumerate() {
        if let Some(group) = groups.iter_mut().find(|group| group.key == row.group) {
            group.rows.push((position, row));
        } else {
            groups.push(Group {
                key: row.group.clone(),
                rows: vec![(position, row)],
            });
        }
    }
    groups
}

/// How many rows are showing, out of how many, and what is filtering them.
#[must_use]
pub fn status(noun: &str, shown: usize, total: usize, query: &str, searching: bool) -> String {
    if searching {
        return format!("/{query}\u{258c}   {shown} of {total}");
    }
    if query.is_empty() {
        let one;
        let noun = if total == 1 {
            one = singular(noun);
            one.as_str()
        } else {
            noun
        };
        return format!("{total} {noun}");
    }
    format!("filter: {query}   {shown} of {total}")
}

/// One of whatever the plural names.
///
/// "1 entries" is the kind of thing a person reads twice, and English is
/// regular enough here to be worth the dozen lines: a listing only ever names
/// what it holds.
#[must_use]
pub fn singular(plural: &str) -> String {
    for (many, one) in [
        ("children", "child"),
        ("people", "person"),
        ("feet", "foot"),
    ] {
        if plural.eq_ignore_ascii_case(many) {
            return one.to_owned();
        }
    }
    if let Some(stem) = plural.strip_suffix("ies") {
        return format!("{stem}y");
    }
    for ending in ["ses", "xes", "zes", "ches", "shes"] {
        if let Some(stem) = plural.strip_suffix(ending) {
            return format!("{stem}{}", &ending[..ending.len() - 2]);
        }
    }
    plural
        .strip_suffix('s')
        .filter(|stem| !stem.ends_with('s'))
        .map_or_else(|| plural.to_owned(), ToOwned::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entries() -> Vec<Row> {
        vec![
            Row::new("a", ["10:32 pm", "Diaper", "pee · big"].map(str::to_owned))
                .group("Sun 27 Sep")
                .tone(Tone::Diaper),
            Row::new(
                "b",
                ["7:00 pm", "Bottle", "34 ml of Breast Milk"].map(str::to_owned),
            )
            .group("Sun 27 Sep")
            .tone(Tone::Feeding),
            Row::new("c", ["9:00 am", "Sleep", "slept 2h 11m"].map(str::to_owned))
                .group("Sat 26 Sep")
                .tone(Tone::Sleep)
                .note(Some("in the carrier".to_owned())),
        ]
    }

    #[test]
    fn a_search_looks_at_every_cell_the_group_and_the_note() {
        let rows = entries();
        assert_eq!(filter(&rows, "diaper").len(), 1);
        assert_eq!(filter(&rows, "27 sep").len(), 2, "the group is searchable");
        assert_eq!(filter(&rows, "carrier").len(), 1, "so is the note");
    }

    #[test]
    fn every_term_has_to_match_so_a_search_narrows() {
        let rows = entries();
        assert_eq!(filter(&rows, "breast milk").len(), 1);
        assert_eq!(filter(&rows, "diaper bottle").len(), 0);
        assert_eq!(filter(&rows, "").len(), 3, "an empty query is no filter");
    }

    #[test]
    fn a_search_does_not_care_about_case() {
        assert_eq!(filter(&entries(), "DIAPER").len(), 1);
    }

    #[test]
    fn groups_keep_the_order_they_first_appear_in() {
        let rows = entries();
        let filtered = filter(&rows, "");
        let groups = grouped(&filtered);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].key, "Sun 27 Sep");
        assert_eq!(groups[0].rows.len(), 2);
        assert_eq!(groups[1].key, "Sat 26 Sep");
    }

    #[test]
    fn a_group_keeps_its_heading_while_one_of_its_rows_survives() {
        let rows = entries();
        let filtered = filter(&rows, "sleep");
        let groups = grouped(&filtered);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].key, "Sat 26 Sep");
    }

    #[test]
    fn a_rows_place_in_the_filtered_list_is_what_the_cursor_addresses() {
        let rows = entries();
        let filtered = filter(&rows, "");
        let groups = grouped(&filtered);
        let places: Vec<usize> = groups
            .iter()
            .flat_map(|group| group.rows.iter().map(|(place, _)| *place))
            .collect();
        assert_eq!(places, vec![0, 1, 2]);
    }

    #[test]
    fn the_status_line_counts_and_says_what_is_filtering() {
        assert_eq!(status("entries", 3, 3, "", false), "3 entries");
        assert_eq!(status("entries", 1, 1, "", false), "1 entry");
        assert_eq!(
            status("entries", 1, 3, "diaper", false),
            "filter: diaper   1 of 3"
        );
        assert!(status("entries", 1, 3, "dia", true).starts_with("/dia"));
    }

    #[test]
    fn one_of_something_is_named_in_the_singular() {
        assert_eq!(singular("entries"), "entry");
        assert_eq!(singular("diapers"), "diaper");
        assert_eq!(singular("children"), "child");
        assert_eq!(singular("foods"), "food");
        assert_eq!(singular("boxes"), "box");
        assert_eq!(singular("sleep"), "sleep", "a word that is already one");
    }

    #[test]
    fn a_kind_cell_takes_the_rows_colour_and_everything_else_its_own() {
        assert_eq!(Role::Kind.tone(Some(Tone::Diaper)), Tone::Diaper);
        assert_eq!(Role::Kind.tone(None), Tone::Accent);
        assert_eq!(Role::Value.tone(Some(Tone::Diaper)), Tone::Value);
    }
}

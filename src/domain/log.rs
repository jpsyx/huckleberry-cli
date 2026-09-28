//! One merged, newest-first stream of everything.
//!
//! This is the screen a parent opens when the totals look wrong and they
//! suspect a feed went unlogged, so it must lose nothing: every row of every
//! collection appears exactly once, in time order, and nothing is summarized
//! away.

use huckleberry_api::RowRef;

use super::types::{Dataset, FeedEvent};

/// Which tracker a row came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A sleep.
    Sleep,
    /// A feed of any sort.
    Feed,
    /// A nappy or a potty trip.
    Diaper,
    /// A pumping session.
    Pump,
    /// A milestone.
    Milestone,
}

impl Kind {
    /// The word `--kind` takes.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sleep => "sleep",
            Self::Feed => "feed",
            Self::Diaper => "diaper",
            Self::Pump => "pump",
            Self::Milestone => "milestone",
        }
    }

    /// Every kind, in the order a chooser should offer them.
    pub const ALL: [Self; 5] = [
        Self::Sleep,
        Self::Feed,
        Self::Diaper,
        Self::Pump,
        Self::Milestone,
    ];

    /// Reads one of the words above.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == text.trim().to_lowercase())
    }
}

/// One row of the stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// A stable identifier.
    pub id: String,
    /// Where the row lives in Huckleberry, when it came from there. What
    /// `edit` needs and what a snapshot read off disk does not have.
    pub at: Option<RowRef>,
    /// Which tracker it came from.
    pub kind: Kind,
    /// When it was.
    pub start: f64,
    /// What to call it.
    pub title: String,
    /// What happened, in a few words.
    pub description: String,
    /// Whatever the parent typed.
    pub notes: Option<String>,
}

/// Everything, newest first.
#[must_use]
pub fn build(dataset: &Dataset) -> Vec<Entry> {
    let mut entries = Vec::new();

    for sleep in &dataset.sleep {
        entries.push(Entry {
            id: format!("sleep:{}", sleep.id),
            at: sleep.at.clone(),
            kind: Kind::Sleep,
            start: sleep.start,
            title: "Sleep".to_owned(),
            description: format!("slept {}", super::time::format_duration(sleep.duration)),
            notes: sleep.notes.clone(),
        });
    }

    for feed in &dataset.feeds {
        let (title, description) = describe_feed(feed);
        entries.push(Entry {
            id: format!("feed:{}", feed.id()),
            at: feed.at().cloned(),
            kind: Kind::Feed,
            start: feed.start(),
            title,
            description,
            notes: feed.notes().map(ToOwned::to_owned),
        });
    }

    for nappy in &dataset.diapers {
        let extras: Vec<&str> = [nappy.color.as_deref(), nappy.consistency.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        let mut description = nappy.mode.clone();
        if !extras.is_empty() {
            description.push_str(" (");
            description.push_str(&extras.join(", "));
            description.push(')');
        }
        if nappy.rash {
            description.push_str(" · rash noted");
        }
        entries.push(Entry {
            id: format!("diaper:{}", nappy.id),
            at: nappy.at.clone(),
            kind: Kind::Diaper,
            start: nappy.start,
            title: if nappy.potty { "Potty" } else { "Nappy" }.to_owned(),
            description,
            notes: nappy.notes.clone(),
        });
    }

    for session in &dataset.pumps {
        let amount = |value: Option<f64>| {
            value.map_or_else(|| "?".to_owned(), |millilitres| format!("{millilitres:.0}"))
        };
        let mut description = format!(
            "{} ml (L {}, R {})",
            amount(session.total_ml),
            amount(session.left_ml),
            amount(session.right_ml)
        );
        if let Some(seconds) = session.duration_seconds {
            description.push_str(" over ");
            description.push_str(&super::time::format_duration(seconds));
        }
        entries.push(Entry {
            id: format!("pump:{}", session.id),
            at: session.at.clone(),
            kind: Kind::Pump,
            start: session.start,
            title: "Pumping".to_owned(),
            description,
            notes: session.notes.clone(),
        });
    }

    for milestone in &dataset.milestones {
        entries.push(Entry {
            id: format!("milestone:{}", milestone.id),
            at: milestone.at.clone(),
            kind: Kind::Milestone,
            start: milestone.start,
            title: "Milestone".to_owned(),
            description: milestone.name.clone(),
            notes: milestone.notes.clone(),
        });
    }

    entries.sort_by(|left, right| {
        right
            .start
            .partial_cmp(&left.start)
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    entries
}

fn describe_feed(feed: &FeedEvent) -> (String, String) {
    match feed {
        FeedEvent::Bottle {
            amount_ml,
            bottle_type,
            ..
        } => {
            let amount = amount_ml.map_or_else(|| "?".to_owned(), |value| format!("{value:.0}"));
            (
                "Bottle".to_owned(),
                format!(
                    "{amount} ml of {}",
                    bottle_type.as_deref().unwrap_or("milk")
                ),
            )
        }
        FeedEvent::Nursing {
            left_seconds,
            right_seconds,
            ..
        } => {
            let total = super::time::format_duration(left_seconds + right_seconds);
            let sides = if *left_seconds > 0.0 || *right_seconds > 0.0 {
                format!(
                    " (L {:.0}m, R {:.0}m)",
                    left_seconds / 60.0,
                    right_seconds / 60.0
                )
            } else {
                String::new()
            };
            ("Nursing".to_owned(), format!("nursed {total}{sides}"))
        }
        FeedEvent::Solids {
            foods, reaction, ..
        } => {
            let what = if foods.is_empty() {
                "solids".to_owned()
            } else {
                foods.join(", ")
            };
            let how = reaction
                .as_deref()
                .map(|taken| format!(" · {}", taken.to_lowercase()))
                .unwrap_or_default();
            ("Solids".to_owned(), format!("{what}{how}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{AFTERNOON, bottle, dataset, nappy, sleep};
    use super::*;
    use crate::domain::types::{MilestoneEvent, PumpEvent};

    #[test]
    fn everything_appears_exactly_once() {
        let mut data = dataset();
        data.sleep = vec![sleep(AFTERNOON - 7200.0, 3600.0)];
        data.feeds = vec![bottle(AFTERNOON - 3600.0, 90.0)];
        data.diapers = vec![nappy(AFTERNOON - 1800.0, true, false)];
        data.pumps = vec![PumpEvent {
            at: None,
            id: "p1".to_owned(),
            start: AFTERNOON - 900.0,
            left_ml: Some(60.0),
            right_ml: Some(40.0),
            total_ml: Some(100.0),
            duration_seconds: Some(900.0),
            notes: None,
        }];
        data.milestones = vec![MilestoneEvent {
            at: None,
            id: "m1".to_owned(),
            start: AFTERNOON - 300.0,
            name: "First smile".to_owned(),
            category: None,
            notes: None,
            has_photo: false,
        }];
        assert_eq!(build(&data).len(), 5);
    }

    #[test]
    fn the_stream_is_newest_first() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 7200.0, 60.0),
            bottle(AFTERNOON - 900.0, 90.0),
        ];
        let entries = build(&data);
        assert!(entries[0].start > entries[1].start);
    }

    #[test]
    fn a_bottle_with_no_amount_says_so_rather_than_saying_zero() {
        let mut data = dataset();
        data.feeds = vec![crate::domain::types::FeedEvent::Bottle {
            at: None,
            id: "b1".to_owned(),
            start: AFTERNOON,
            amount_ml: None,
            bottle_type: Some("Formula".to_owned()),
            notes: None,
        }];
        assert_eq!(build(&data)[0].description, "? ml of Formula");
    }

    #[test]
    fn a_potty_trip_is_titled_as_one() {
        let mut data = dataset();
        let mut trip = nappy(AFTERNOON, true, false);
        trip.potty = true;
        data.diapers = vec![trip];
        assert_eq!(build(&data)[0].title, "Potty");
    }

    #[test]
    fn a_rash_is_carried_into_the_description() {
        let mut data = dataset();
        let mut sore = nappy(AFTERNOON, true, true);
        sore.rash = true;
        sore.color = Some("yellow".to_owned());
        data.diapers = vec![sore];
        let description = &build(&data)[0].description;
        assert!(description.contains("yellow"), "{description}");
        assert!(description.contains("rash noted"), "{description}");
    }

    #[test]
    fn an_entry_carries_the_row_it_came_from_so_it_can_be_edited() {
        let mut data = dataset();
        let mut event = nappy(AFTERNOON, true, false);
        event.at = Some(RowRef::loose("diaper", "row-one"));
        data.diapers = vec![event];
        assert_eq!(build(&data)[0].at, Some(RowRef::loose("diaper", "row-one")));
    }

    #[test]
    fn an_entry_read_off_a_snapshot_has_no_row_to_edit() {
        let mut data = dataset();
        data.diapers = vec![nappy(AFTERNOON, true, false)];
        assert_eq!(build(&data)[0].at, None);
    }

    #[test]
    fn every_kind_has_a_word_and_reads_back_from_it() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::parse("SLEEP"), Some(Kind::Sleep));
        assert_eq!(Kind::parse("nappies"), None);
    }
}

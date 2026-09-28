//! The merged stream, as lines of text.
//!
//! Grouped under a heading per day, because a flat list of forty rows with the
//! date on every one is harder to scan than the same rows under four dates.

use crate::domain::log::{Entry, Kind};
use crate::domain::time::Calendar;
use crate::theme::{Theme, Tone};

use super::format;

/// How wide the title column is.
const TITLE_WIDTH: usize = 9;

/// The colour each kind of entry is drawn in.
///
/// One mapping, so the one-shot stream and the full-screen picker agree: a
/// diaper is the same colour wherever a person meets it.
#[must_use]
pub const fn tone_for(kind: Kind) -> Tone {
    match kind {
        Kind::Sleep => Tone::Sleep,
        Kind::Feed => Tone::Feeding,
        Kind::Diaper => Tone::Diaper,
        Kind::Pump => Tone::Pumping,
        Kind::Milestone => Tone::Milestone,
    }
}

/// The stream, newest first, grouped by day. `now` decides which heading
/// is today's.
#[must_use]
pub fn lines(
    entries: &[Entry],
    calendar: &Calendar,
    theme: Theme,
    limit: usize,
    now: f64,
) -> Vec<String> {
    if entries.is_empty() {
        return vec![theme.muted("nothing logged in this window")];
    }

    let today = calendar.day_of(now);
    let mut lines = Vec::new();
    let mut current_day = None;
    for entry in entries.iter().take(limit) {
        let day = calendar.day_of(entry.start);
        if current_day != Some(day) {
            if current_day.is_some() {
                lines.push(String::new());
            }
            // Today's heading is the brightest on the screen, as it is
            // everywhere else a list of days appears.
            let label = format::day_short(day);
            lines.push(if day == today {
                theme.today(&label)
            } else {
                theme.heading(&label)
            });
            current_day = Some(day);
        }
        lines.push(row(entry, calendar, theme));
        if let Some(notes) = &entry.notes {
            lines.push(theme.muted(&format!("{}{notes}", " ".repeat(TITLE_WIDTH + 10))));
        }
    }

    if entries.len() > limit {
        lines.push(String::new());
        lines.push(theme.muted(&format!(
            "{} more · pass --limit to see them",
            entries.len() - limit
        )));
    }
    lines
}

/// One entry's line.
#[must_use]
pub fn row(entry: &Entry, calendar: &Calendar, theme: Theme) -> String {
    format!(
        "  {}  {}  {}",
        theme.muted(&format::pad_left(&format::clock(entry.start, calendar), 8)),
        theme.paint(
            tone_for(entry.kind),
            &format::pad(&entry.title, TITLE_WIDTH)
        ),
        theme.value(&entry.description)
    )
}

/// Only the entries of one kind, when a kind was asked for.
#[must_use]
pub fn only(entries: Vec<Entry>, kind: Option<Kind>) -> Vec<Entry> {
    match kind {
        None => entries,
        Some(wanted) => entries
            .into_iter()
            .filter(|entry| entry.kind == wanted)
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::fixtures::{AFTERNOON, bottle, dataset, diaper, sleep};
    use crate::domain::log;

    fn calendar() -> Calendar {
        Calendar::new("America/New_York").expect("a real timezone")
    }

    fn rendered(data: &crate::domain::types::Dataset, limit: usize) -> String {
        lines(
            &log::build(data),
            &calendar(),
            Theme::dark(false),
            limit,
            AFTERNOON,
        )
        .join("\n")
    }

    #[test]
    fn an_empty_window_says_so() {
        assert!(rendered(&dataset(), 40).contains("nothing logged"));
    }

    #[test]
    fn entries_are_grouped_under_the_day_they_happened_on() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 3_600.0, 90.0),
            bottle(AFTERNOON - 86_400.0, 60.0),
        ];
        let text = rendered(&data, 40);
        assert!(text.contains("Mon 22 Sep"), "{text}");
        assert!(text.contains("Sun 21 Sep"), "{text}");
    }

    #[test]
    fn a_note_is_shown_under_the_entry_it_belongs_to() {
        let mut data = dataset();
        let mut noted = diaper(AFTERNOON - 900.0, true, false);
        noted.notes = Some("a bit sore".to_owned());
        data.diapers = vec![noted];
        let text = rendered(&data, 40);
        assert!(text.contains("a bit sore"), "{text}");
    }

    #[test]
    fn a_limit_says_how_many_were_left_out() {
        let mut data = dataset();
        data.feeds = (0..5)
            .map(|index| bottle(AFTERNOON - f64::from(index) * 3_600.0, 90.0))
            .collect();
        let text = rendered(&data, 2);
        assert!(text.contains("3 more"), "{text}");
        assert!(text.contains("--limit"), "{text}");
    }

    #[test]
    fn a_window_inside_the_limit_says_nothing_about_more() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        assert!(!rendered(&data, 40).contains("more ·"));
    }

    #[test]
    fn a_kind_filter_keeps_only_that_kind() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        data.sleep = vec![sleep(AFTERNOON - 7_200.0, 3_600.0)];
        let sleeps = only(log::build(&data), Some(Kind::Sleep));
        assert_eq!(sleeps.len(), 1);
        assert_eq!(sleeps[0].kind, Kind::Sleep);
    }

    #[test]
    fn no_filter_keeps_everything() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        data.sleep = vec![sleep(AFTERNOON - 7_200.0, 3_600.0)];
        assert_eq!(only(log::build(&data), None).len(), 2);
    }

    #[test]
    fn every_kind_of_entry_is_drawn_in_its_own_colour() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        data.diapers = vec![diaper(AFTERNOON - 60.0, true, false)];
        data.sleep = vec![sleep(AFTERNOON - 7_200.0, 3_600.0)];
        let entries = log::build(&data);
        let painted: Vec<String> = entries
            .iter()
            .map(|entry| row(entry, &calendar(), Theme::dark(true)))
            .collect();
        for (entry, line) in entries.iter().zip(&painted) {
            assert!(
                line.contains(tone_for(entry.kind).sgr()),
                "{:?} is not in its own colour: {line:?}",
                entry.kind
            );
        }
    }

    #[test]
    fn a_row_leads_with_the_time_it_happened() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        let text = rendered(&data, 40);
        assert!(text.contains("2:00 pm"), "{text}");
        assert!(text.contains("Bottle"), "{text}");
    }
}

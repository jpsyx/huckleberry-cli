//! The merged stream, as lines of text.
//!
//! Grouped under a heading per day, because a flat list of forty rows with the
//! date on every one is harder to scan than the same rows under four dates.

use crate::domain::log::{Entry, Kind};
use crate::domain::time::Calendar;
use crate::edit;
use crate::listing::{Column, Role, Row};
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

/// The columns the stream is listed in.
pub const COLUMNS: [Column; 3] = [
    Column::new("when", Role::Key),
    Column::new("what", Role::Kind),
    Column::new("detail", Role::Value),
];

/// The stream as rows of a listing, grouped by the day they happened on.
///
/// `refusal` says which entries the command cannot act on, and why; a row it
/// refuses is still listed, because the stream is the stream.
#[must_use]
pub fn rows(
    entries: &[Entry],
    calendar: &Calendar,
    refusal: &dyn Fn(&Entry) -> Option<String>,
) -> Vec<Row> {
    entries
        .iter()
        .map(|entry| {
            let row = Row::new(
                entry
                    .at
                    .as_ref()
                    .map_or_else(|| entry.id.clone(), edit::token_for),
                [
                    format::clock(entry.start, calendar),
                    entry.title.clone(),
                    entry.description.clone(),
                ],
            )
            .group(format::day_short(calendar.day_of(entry.start)))
            .tone(tone_for(entry.kind))
            .note(entry.notes.clone())
            .detail(detail(entry, calendar));
            match refusal(entry) {
                Some(why) => row.refused(why),
                None => row,
            }
        })
        .collect()
}

/// What is known about one entry, for the screen that shows one.
fn detail(entry: &Entry, calendar: &Calendar) -> Vec<(String, String)> {
    let mut pairs = vec![
        (
            "when".to_owned(),
            format!(
                "{} {}",
                format::day_short(calendar.day_of(entry.start)),
                format::clock(entry.start, calendar)
            ),
        ),
        ("what".to_owned(), entry.title.clone()),
        ("detail".to_owned(), entry.description.clone()),
    ];
    if let Some(notes) = &entry.notes {
        pairs.push(("notes".to_owned(), notes.clone()));
    }
    if let Some(at) = &entry.at {
        pairs.push(("entry".to_owned(), edit::token_for(at)));
    }
    pairs
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
    fn a_row_carries_the_entrys_name_its_colour_its_note_and_its_details() {
        let mut data = dataset();
        let mut noted = diaper(AFTERNOON, true, false);
        noted.at = Some(huckleberry_api::RowRef::loose("diaper", "row-one"));
        noted.notes = Some("a bit sore".to_owned());
        data.diapers = vec![noted];
        let entries = log::build(&data);
        let rows = rows(&entries, &calendar(), &|_| None);

        assert_eq!(rows[0].key, "diaper/row-one", "the name `edit` knows it by");
        assert_eq!(rows[0].tone, Some(Tone::Diaper));
        assert_eq!(rows[0].note.as_deref(), Some("a bit sore"));
        assert_eq!(rows[0].group, "Mon 22 Sep");
        assert!(rows[0].selectable);
        assert!(
            rows[0]
                .detail
                .iter()
                .any(|(label, value)| label == "entry" && value == "diaper/row-one"),
            "{:?}",
            rows[0].detail
        );
    }

    #[test]
    fn a_row_the_command_cannot_act_on_is_listed_and_says_why() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        let entries = log::build(&data);
        let rows = rows(&entries, &calendar(), &|_| Some("not here".to_owned()));
        assert!(!rows[0].selectable);
        assert_eq!(rows[0].refusal.as_deref(), Some("not here"));
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

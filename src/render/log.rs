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
        Kind::Health => Tone::Info,
    }
}

/// The columns the stream is listed in.
pub const COLUMNS: [Column; 3] = [
    Column::new("when", Role::Key),
    Column::new("what", Role::Kind),
    Column::new("detail", Role::Value),
];

/// How recent an entry has to be to carry how long ago it was.
const RECENT: f64 = 6.0 * 3_600.0;

/// The time a row leads with: the clock, and for a recent entry how long ago.
///
/// Six hours is the window in which "how long has it been?" is the question
/// somebody is actually asking of a list. Past that the clock time is the
/// answer, and the parenthesis is noise in every row.
fn when(at: f64, now: f64, calendar: &Calendar) -> String {
    let clock = format::clock(at, calendar);
    let elapsed = now - at;
    // A clock a little out of step with Huckleberry's can date an entry
    // seconds ahead; `format_ago` reads that as "just now" rather than hiding it.
    if !(-60.0..=RECENT).contains(&elapsed) {
        return clock;
    }
    format!("{clock} ({})", crate::domain::time::format_ago(at, now))
}

/// The stream as rows of a listing, grouped by the day they happened on.
///
/// `refusal` says which entries the command cannot act on, and why; a row it
/// refuses is still listed, because the stream is the stream. `now` decides
/// which rows are recent enough to say how long ago they were.
#[must_use]
pub fn rows(
    entries: &[Entry],
    calendar: &Calendar,
    now: f64,
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
                    when(entry.start, now, calendar),
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
        let rows = rows(&entries, &calendar(), AFTERNOON, &|_| None);

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
    fn a_recent_row_says_how_long_ago_it_was_beside_the_time() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 9_120.0, 90.0)];
        let rows = rows(&log::build(&data), &calendar(), AFTERNOON, &|_| None);
        assert_eq!(rows[0].cells[0], "11:28 am (2h 32m ago)");
    }

    #[test]
    fn a_row_older_than_six_hours_is_left_with_the_time_alone() {
        let mut data = dataset();
        data.feeds = vec![
            bottle(AFTERNOON - 6.0 * 3_600.0, 90.0),
            bottle(AFTERNOON - 6.0 * 3_600.0 - 1.0, 90.0),
        ];
        let rows = rows(&log::build(&data), &calendar(), AFTERNOON, &|_| None);
        assert_eq!(
            rows[0].cells[0], "8:00 am (6h 0m ago)",
            "six hours is recent"
        );
        assert_eq!(rows[1].cells[0], "7:59 am");
    }

    #[test]
    fn a_kind_starts_in_the_same_column_whether_or_not_a_row_says_how_long_ago() {
        use crate::listing::{layout, model};
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON - 9_120.0, 90.0)];
        data.diapers = vec![diaper(AFTERNOON - 7.0 * 3_600.0, true, false)];
        let rows = rows(&log::build(&data), &calendar(), AFTERNOON, &|_| None);
        let filtered = model::filter(&rows, "");
        let widths = layout::widths(&COLUMNS, &filtered, None);
        let drawn = layout::body_lines(
            &COLUMNS,
            &model::grouped(&filtered),
            &widths,
            &str::to_owned,
            &|_| false,
        );
        let plain: Vec<String> = drawn.iter().map(layout::Line::plain).collect();
        let bottle_at = plain
            .iter()
            .find_map(|line| line.find("Bottle"))
            .expect("the bottle row");
        let diaper_at = plain
            .iter()
            .find_map(|line| line.find("Diaper"))
            .expect("the diaper row");
        assert_eq!(bottle_at, diaper_at, "{plain:?}");
    }

    #[test]
    fn a_row_the_command_cannot_act_on_is_listed_and_says_why() {
        let mut data = dataset();
        data.feeds = vec![bottle(AFTERNOON, 90.0)];
        let entries = log::build(&data);
        let rows = rows(&entries, &calendar(), AFTERNOON, &|_| {
            Some("not here".to_owned())
        });
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

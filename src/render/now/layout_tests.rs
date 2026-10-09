//! Observable placement and wrapping of the three Now sections.

use super::*;
use crate::domain::fixtures::{AFTERNOON, bottle, dataset};
use crate::domain::now;
use crate::domain::today::DayRule;
use ratatui::text::Span;

fn sample() -> Dataset {
    let mut data = dataset();
    data.child.birthdate = Some("2025-09-08".to_owned());
    data.feeds = vec![bottle(AFTERNOON - 3_600.0, 90.0)];
    data
}

fn rendered(data: &Dataset, width: Option<usize>) -> Vec<String> {
    let calendar = Calendar::new("America/New_York").unwrap();
    let view = now::build(data, &calendar, DayRule::continuous(6.0, 19.5), AFTERNOON);
    lines(
        &view,
        data,
        &calendar,
        Theme::dark(false),
        Units::Ml,
        AFTERNOON,
        width,
    )
}

fn position(rows: &[String], label: &str) -> (usize, usize) {
    rows.iter()
        .enumerate()
        .find_map(|(row, text)| {
            text.find(label)
                .map(|offset| (row, Span::raw(&text[..offset]).width()))
        })
        .unwrap_or_else(|| panic!("missing {label}:\n{}", rows.join("\n")))
}

#[test]
fn all_side_by_side_widths_stack_ranges_under_facts_on_the_left() {
    for width in [96, 120, 143, 144, 180, 240] {
        let rows = rendered(&sample(), Some(width));
        let facts = position(&rows, "Last fed");
        let age = position(&rows, "days old");
        let typical = position(&rows, "typical feed");
        let recent = position(&rows, "In last 4h");
        let daily = position(&rows, "In last 24h");
        assert!(facts.1 < recent.1, "{}", rows.join("\n"));
        assert_eq!(facts.1, typical.1, "ranges stay left at {width} cells");
        assert!(age.0 > facts.0 && typical.0 > age.0);
        assert_eq!(recent.1, daily.1);
        assert!(recent.0 < daily.0);
    }
}

#[test]
fn wrapping_keeps_a_fitting_feed_amount_and_type_together() {
    let rows = rendered(&sample(), Some(97));
    assert!(
        rows.iter().any(|row| row.contains("90 ml of Formula")),
        "{}",
        rows.join("\n")
    );
}

#[test]
fn narrow_and_unknown_widths_stack_all_three_sections() {
    for width in [None, Some(40), Some(80), Some(95)] {
        let rows = rendered(&sample(), width);
        let labels = ["Last fed", "In last 4h", "In last 24h", "days old"];
        let positions: Vec<_> = labels.iter().map(|label| position(&rows, label)).collect();
        assert!(positions.windows(2).all(|pair| pair[0].0 < pair[1].0));
        assert_eq!(positions[1].1, 0);
        assert!(rows.iter().any(|row| row.contains("pediatrician")));
    }
}

#[test]
fn known_widths_bound_every_row_in_terminal_cells() {
    let mut data = sample();
    data.child.name = "小熊👶e\u{301}".repeat(20);
    for width in [28, 40, 80, 95, 96, 120, 143, 144, 180, 240] {
        let rows = rendered(&data, Some(width));
        assert!(
            rows.iter().all(|row| Span::raw(row).width() <= width),
            "overflow at {width}:\n{}",
            rows.join("\n")
        );
        for label in [
            "Last fed",
            "Last diaper",
            "Last sleep",
            "In last 4h",
            "In last 24h",
            "pediatrician",
        ] {
            position(&rows, label);
        }
        for glyph in ["小", "熊", "👶", "e\u{301}"] {
            assert_eq!(rows.join("\n").matches(glyph).count(), 20);
        }
    }
}

#[test]
fn without_ranges_uses_two_useful_columns() {
    let mut data = sample();
    data.child.birthdate = None;
    let rows = rendered(&data, Some(180));
    assert!(position(&rows, "Last fed").1 < position(&rows, "In last 4h").1);
    assert!(!rows.join("\n").contains("typical"));
}

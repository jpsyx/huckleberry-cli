//! One turning column. No terminal, no clock, no idea what it is counting.
//!
//! Everything a dial knows how to do lives here: a list of labels, where it
//! is standing, and how far a turn moves it. What the labels mean is the
//! caller's business, which is what lets hours, minutes, millilitres and
//! ounces all be the same component.

/// One column of a dial.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wheel {
    /// What each position reads as, in order.
    labels: Vec<String>,
    /// Which one is chosen.
    index: usize,
    /// How far a shifted turn moves, in positions.
    ///
    /// Minutes leap five at a time, ounces four, which is one ounce. A wheel
    /// with nothing worth leaping over leaps one, the same as a plain turn.
    leap: usize,
    /// What is drawn before this column.
    gap: &'static str,
    /// Whether it comes round at the ends.
    ///
    /// Most columns do: an hour of the day has no first or last one. Some
    /// have a real end, and nothing is less than no hours ago.
    cycles: bool,
}

impl Wheel {
    /// A wheel standing at `index`, clamped into the labels it has.
    ///
    /// # Panics
    ///
    /// When given no labels: a column with nothing to show is a programming
    /// error, not a state a parent can reach.
    #[must_use]
    pub fn new(labels: Vec<String>, index: usize, leap: usize, gap: &'static str) -> Self {
        assert!(!labels.is_empty(), "a wheel needs something to show");
        let index = index.min(labels.len() - 1);
        Self {
            labels,
            index,
            leap: leap.max(1),
            gap,
            cycles: true,
        }
    }

    /// The same wheel, stopping at its ends rather than coming round.
    ///
    /// It draws nothing past them either, so the end of the column looks
    /// like one rather than like a value that failed to load.
    #[must_use]
    pub const fn holding(mut self) -> Self {
        self.cycles = false;
        self
    }

    /// What it reads `offset` places along, wrapping at both ends.
    ///
    /// Always wraps, so no column ever looks like it has run out.
    #[must_use]
    pub fn at(&self, offset: isize) -> &str {
        let standing = isize::try_from(self.index).unwrap_or_default();
        let count = isize::try_from(self.labels.len()).unwrap_or(1);
        let wanted = standing + offset;
        if !self.cycles && (wanted < 0 || wanted >= count) {
            return "";
        }
        self.labels
            .get(usize::try_from(wanted.rem_euclid(count)).unwrap_or_default())
            .map_or("", String::as_str)
    }

    /// What it shows `offset` places along, which may be nothing.
    ///
    /// A cycling wheel of two is a toggle: the one not chosen sits directly
    /// above the one that is, rather than repeating every other row. A
    /// holding wheel keeps each neighbour in its direction of travel, even
    /// with only two values, so increasing still points the same way.
    #[must_use]
    pub fn shown_at(&self, offset: isize) -> &str {
        match self.count() {
            // Nothing to choose between: it is a fact rather than a wheel.
            1 if offset != 0 => "",
            2 if self.cycles && offset != 0 && offset != -1 => "",
            _ => self.at(offset),
        }
    }

    /// What is chosen.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.labels[self.index]
    }

    /// Which position is chosen.
    #[must_use]
    pub const fn index(&self) -> usize {
        self.index
    }

    /// How many positions it has. Never none: see [`Wheel::new`].
    #[must_use]
    pub fn count(&self) -> usize {
        self.labels.len()
    }

    /// How far a shifted turn moves it.
    #[must_use]
    pub const fn leap(&self) -> usize {
        self.leap
    }

    /// What is drawn before this column.
    #[must_use]
    pub const fn gap(&self) -> &'static str {
        self.gap
    }

    /// The widest label, which is how wide the column is drawn.
    #[must_use]
    pub fn width(&self) -> usize {
        self.labels
            .iter()
            .map(|label| label.chars().count())
            .max()
            .unwrap_or(1)
    }

    /// Stands it on a label, if it has one. Answers whether it moved.
    pub fn stand_on(&mut self, label: &str) -> bool {
        let Some(found) = self.labels.iter().position(|value| value == label) else {
            return false;
        };
        let moved = found != self.index;
        self.index = found;
        moved
    }

    /// Turns it, wrapping at both ends.
    pub fn turn(&mut self, steps: isize) {
        let standing = isize::try_from(self.index).unwrap_or_default();
        let count = isize::try_from(self.labels.len()).unwrap_or(1);
        let wanted = standing + steps;
        let landed = if self.cycles {
            wanted.rem_euclid(count)
        } else {
            wanted.clamp(0, count - 1)
        };
        self.index = usize::try_from(landed).unwrap_or_default();
    }

    /// Stands it at the label nearest `wanted`, by whatever `distance` says
    /// near means.
    ///
    /// Used when a wheel is rebuilt under a value that has to survive it: an
    /// amount in millilitres becoming the same amount in ounces lands on the
    /// nearest quarter rather than on wherever the old position pointed.
    pub fn stand_nearest(&mut self, distance: impl Fn(&str) -> f64) {
        self.index = self
            .labels
            .iter()
            .enumerate()
            .min_by(|(_, left), (_, right)| {
                distance(left)
                    .partial_cmp(&distance(right))
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            .map_or(0, |(index, _)| index);
    }

    /// How far it is from one position to another, the short way round.
    ///
    /// Signed, so crossing the end of a wheel reads as a few places forward
    /// rather than as most of a turn back, which is what it looked like to
    /// whoever turned it.
    #[must_use]
    pub fn shortest_way(&self, from: usize) -> isize {
        let count = isize::try_from(self.labels.len()).unwrap_or(1);
        let standing = isize::try_from(self.index).unwrap_or_default();
        let was = isize::try_from(from).unwrap_or_default();
        if !self.cycles {
            return standing - was;
        }
        let forward = (standing - was).rem_euclid(count);
        if forward > count / 2 {
            forward - count
        } else {
            forward
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbers(count: usize, leap: usize) -> Wheel {
        Wheel::new(
            (0..count).map(|value| value.to_string()).collect(),
            0,
            leap,
            "",
        )
    }

    /// Some columns have a real end. Nothing is less than no hours ago, and
    /// a wheel that turns from nought to twenty-three on one press is a
    /// wheel that logs yesterday by accident.
    #[test]
    fn a_holding_wheel_stops_at_its_ends_rather_than_coming_round() {
        let mut wheel = numbers(24, 1).holding();
        wheel.turn(-1);
        assert_eq!(wheel.value(), "0", "there is nothing above nought");
        wheel.turn(5);
        assert_eq!(wheel.value(), "5");
        wheel.turn(100);
        assert_eq!(wheel.value(), "23", "and nothing below the last one");
    }

    /// And shows nothing past them, so the end of the column looks like one.
    #[test]
    fn a_holding_wheel_draws_nothing_past_its_ends() {
        let wheel = numbers(24, 1).holding();
        assert_eq!(wheel.shown_at(0), "0");
        assert_eq!(wheel.shown_at(1), "1");
        assert_eq!(wheel.shown_at(-1), "", "nothing above it");
        assert_eq!(wheel.shown_at(-2), "");
    }

    #[test]
    fn a_wheel_that_was_not_told_to_hold_still_comes_round() {
        let mut wheel = numbers(60, 5);
        wheel.turn(-1);
        assert_eq!(wheel.value(), "59");
    }

    #[test]
    fn a_wheel_wraps_at_both_ends_so_it_never_runs_out() {
        let wheel = numbers(60, 5);
        assert_eq!(wheel.at(0), "0");
        assert_eq!(wheel.at(-1), "59");
        assert_eq!(wheel.at(-2), "58");
        assert_eq!(wheel.at(1), "1");
        assert_eq!(wheel.at(61), "1", "and keeps wrapping");
    }

    #[test]
    fn turning_it_wraps_the_same_way() {
        let mut wheel = numbers(12, 1);
        wheel.turn(-1);
        assert_eq!(wheel.value(), "11");
        wheel.turn(2);
        assert_eq!(wheel.value(), "1");
    }

    #[test]
    fn a_wheel_is_as_wide_as_its_widest_label() {
        let wheel = Wheel::new(vec!["1".into(), "12".into()], 0, 1, "");
        assert_eq!(wheel.width(), 2);
    }

    #[test]
    fn an_index_past_the_end_is_brought_back_inside() {
        let wheel = Wheel::new(vec!["a".into(), "b".into()], 9, 1, "");
        assert_eq!(wheel.value(), "b");
    }

    /// A wheel of two is a toggle: either way round reaches the other one.
    #[test]
    fn a_wheel_of_two_is_a_toggle_whichever_way_it_turns() {
        let mut wheel = Wheel::new(vec!["am".into(), "pm".into()], 0, 1, "");
        wheel.turn(1);
        assert_eq!(wheel.value(), "pm");
        wheel.turn(1);
        assert_eq!(wheel.value(), "am");
        wheel.turn(-1);
        assert_eq!(wheel.value(), "pm");
    }

    #[test]
    fn the_short_way_round_is_signed_so_crossing_the_end_reads_as_a_few_places() {
        let mut wheel = numbers(60, 5);
        wheel.turn(5);
        assert_eq!(wheel.shortest_way(0), 5);
        let mut wheel = numbers(60, 5);
        wheel.turn(-5);
        assert_eq!(wheel.shortest_way(0), -5, "and not fifty-five forward");
    }

    #[test]
    fn a_rebuilt_wheel_stands_at_whatever_is_nearest_the_value_it_held() {
        let mut wheel = Wheel::new(
            (0..=8)
                .map(|step| format!("{:.2}", f64::from(step) * 0.25))
                .collect(),
            0,
            4,
            "",
        );
        // Four ounces and a bit: the nearest quarter is the one to stand on.
        wheel.stand_nearest(|label| (label.parse::<f64>().unwrap_or_default() - 1.1).abs());
        assert_eq!(wheel.value(), "1.00");
        wheel.stand_nearest(|label| (label.parse::<f64>().unwrap_or_default() - 1.4).abs());
        assert_eq!(wheel.value(), "1.50");
    }

    #[test]
    fn a_leap_of_nothing_is_a_leap_of_one() {
        assert_eq!(numbers(5, 0).leap(), 1);
    }
}

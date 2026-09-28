//! The fixed sets of words the flags accept.
//!
//! Every one of these mirrors a Huckleberry value, and declaring it as a
//! `ValueEnum` rather than taking a `String` buys three things at once: the
//! help text lists the choices, a typo is refused by the parser with the list
//! rather than by the server with a shrug, and shell completion works.
//!
//! The spellings here are the command line's, not Huckleberry's. A person
//! types `--type breast-milk`; the app stores `Breast Milk`. `to_api` is the
//! only place that knows both.

use clap::ValueEnum;
use huckleberry_api::models::diaper::{
    DiaperAmount, DiaperMode, PooColor, PooConsistency, PottyResult,
};
use huckleberry_api::models::feed::{BottleType, FeedSide, VolumeUnits};
use huckleberry_api::models::health::MeasurementSystem;
use huckleberry_api::models::solids::SolidsReaction;

/// What was in the bottle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum BottleKind {
    /// Formula.
    Formula,
    /// Expressed breast milk.
    BreastMilk,
    /// Cow milk.
    CowMilk,
    /// Goat milk.
    GoatMilk,
    /// Soy milk.
    SoyMilk,
    /// Tube feeding.
    TubeFeeding,
    /// Something else.
    Other,
}

impl BottleKind {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> BottleType {
        match self {
            Self::Formula => BottleType::Formula,
            Self::BreastMilk => BottleType::BreastMilk,
            Self::CowMilk => BottleType::CowMilk,
            Self::GoatMilk => BottleType::GoatMilk,
            Self::SoyMilk => BottleType::SoyMilk,
            Self::TubeFeeding => BottleType::TubeFeeding,
            Self::Other => BottleType::Other,
        }
    }
}

/// How a volume is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Units {
    /// Millilitres.
    Ml,
    /// Fluid ounces.
    Oz,
}

impl Units {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> VolumeUnits {
        match self {
            Self::Ml => VolumeUnits::Millilitres,
            Self::Oz => VolumeUnits::Ounces,
        }
    }

    /// The setting spelling, which is what `config set units` stores.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ml => "ml",
            Self::Oz => "oz",
        }
    }

    /// Reads the setting spelling back.
    #[must_use]
    pub fn from_setting(value: &str) -> Self {
        if value.eq_ignore_ascii_case("oz") {
            Self::Oz
        } else {
            Self::Ml
        }
    }
}

/// Which breast.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Side {
    /// The left.
    Left,
    /// The right.
    Right,
}

impl Side {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> FeedSide {
        match self {
            Self::Left => FeedSide::Left,
            Self::Right => FeedSide::Right,
        }
    }
}

/// What was in the nappy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum NappyKind {
    /// Wet only.
    Pee,
    /// Dirty only.
    Poo,
    /// Both.
    Both,
    /// Neither.
    Dry,
}

impl NappyKind {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> DiaperMode {
        match self {
            Self::Pee => DiaperMode::Pee,
            Self::Poo => DiaperMode::Poo,
            Self::Both => DiaperMode::Both,
            Self::Dry => DiaperMode::Dry,
        }
    }
}

/// How much, as the app's three buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Amount {
    /// The app's "little".
    Little,
    /// The app's "medium".
    Medium,
    /// The app's "big".
    Big,
}

impl Amount {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> DiaperAmount {
        match self {
            Self::Little => DiaperAmount::Little,
            Self::Medium => DiaperAmount::Medium,
            Self::Big => DiaperAmount::Big,
        }
    }
}

/// The colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Colour {
    /// Yellow.
    Yellow,
    /// Brown.
    Brown,
    /// Black.
    Black,
    /// Green.
    Green,
    /// Red.
    Red,
    /// Grey.
    Gray,
}

impl Colour {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> PooColor {
        match self {
            Self::Yellow => PooColor::Yellow,
            Self::Brown => PooColor::Brown,
            Self::Black => PooColor::Black,
            Self::Green => PooColor::Green,
            Self::Red => PooColor::Red,
            Self::Gray => PooColor::Gray,
        }
    }
}

/// The consistency.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Consistency {
    /// Solid.
    Solid,
    /// Loose.
    Loose,
    /// Runny.
    Runny,
    /// Mucousy.
    Mucousy,
    /// Hard.
    Hard,
    /// Pebbles.
    Pebbles,
    /// Diarrhea.
    Diarrhea,
}

impl Consistency {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> PooConsistency {
        match self {
            Self::Solid => PooConsistency::Solid,
            Self::Loose => PooConsistency::Loose,
            Self::Runny => PooConsistency::Runny,
            Self::Mucousy => PooConsistency::Mucousy,
            Self::Hard => PooConsistency::Hard,
            Self::Pebbles => PooConsistency::Pebbles,
            Self::Diarrhea => PooConsistency::Diarrhea,
        }
    }
}

/// How a potty trip went.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PottyOutcome {
    /// Sat, but nothing happened.
    SatButDry,
    /// Went.
    WentPotty,
    /// Did not make it.
    Accident,
}

impl PottyOutcome {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> PottyResult {
        match self {
            Self::SatButDry => PottyResult::SatButDry,
            Self::WentPotty => PottyResult::WentPotty,
            Self::Accident => PottyResult::Accident,
        }
    }
}

/// How the baby took a meal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Reaction {
    /// Loved it.
    Loved,
    /// Took it or left it.
    Meh,
    /// Hated it.
    Hated,
    /// Reacted badly.
    Allergic,
}

impl Reaction {
    /// The value Huckleberry stores.
    #[must_use]
    pub const fn to_api(self) -> SolidsReaction {
        match self {
            Self::Loved => SolidsReaction::Loved,
            Self::Meh => SolidsReaction::Meh,
            Self::Hated => SolidsReaction::Hated,
            Self::Allergic => SolidsReaction::Allergic,
        }
    }
}

/// Which system growth measurements are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum System {
    /// Kilograms and centimetres.
    Metric,
    /// Pounds and inches.
    Imperial,
}

impl System {
    /// The value the API takes.
    #[must_use]
    pub const fn to_api(self) -> MeasurementSystem {
        match self {
            Self::Metric => MeasurementSystem::Metric,
            Self::Imperial => MeasurementSystem::Imperial,
        }
    }

    /// Reads the setting spelling.
    #[must_use]
    pub fn from_setting(value: &str) -> Self {
        if value.eq_ignore_ascii_case("imperial") {
            Self::Imperial
        } else {
            Self::Metric
        }
    }
}

/// What a listing should be sorted or filtered to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogKind {
    /// Sleeps.
    Sleep,
    /// Feeds of any sort.
    Feed,
    /// Nappies and potty trips.
    Diaper,
    /// Pumping sessions.
    Pump,
    /// Milestones.
    Milestone,
}

impl LogKind {
    /// The domain's own word for this kind.
    #[must_use]
    pub const fn to_domain(self) -> crate::domain::log::Kind {
        match self {
            Self::Sleep => crate::domain::log::Kind::Sleep,
            Self::Feed => crate::domain::log::Kind::Feed,
            Self::Diaper => crate::domain::log::Kind::Diaper,
            Self::Pump => crate::domain::log::Kind::Pump,
            Self::Milestone => crate::domain::log::Kind::Milestone,
        }
    }
}

/// Which trend to chart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TrendMetric {
    /// Millilitres by bottle per day.
    Milk,
    /// Milk feeds per day.
    Feeds,
    /// Total sleep per day.
    Sleep,
    /// Sleep inside the night window, per day.
    NightSleep,
    /// The longest unbroken sleep per day.
    LongestSleep,
    /// Wet nappies per day.
    Wet,
    /// Dirty nappies per day.
    Dirty,
    /// Minutes nursing per day.
    Nursing,
    /// Millilitres expressed per day.
    Pumped,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_line_spelling_becomes_the_apps_own() {
        assert_eq!(BottleKind::BreastMilk.to_api().as_str(), "Breast Milk");
        assert_eq!(BottleKind::TubeFeeding.to_api().as_str(), "Tube Feeding");
        assert_eq!(PottyOutcome::WentPotty.to_api().as_str(), "wentPotty");
        assert_eq!(Reaction::Loved.to_api().as_str(), "LOVED");
    }

    #[test]
    fn every_bottle_kind_maps_to_a_value_the_app_knows() {
        for kind in [
            BottleKind::Formula,
            BottleKind::BreastMilk,
            BottleKind::CowMilk,
            BottleKind::GoatMilk,
            BottleKind::SoyMilk,
            BottleKind::TubeFeeding,
            BottleKind::Other,
        ] {
            assert!(kind.to_api().is_known(), "{kind:?}");
        }
    }

    #[test]
    fn every_nappy_value_maps_to_one_the_app_knows() {
        for kind in [
            NappyKind::Pee,
            NappyKind::Poo,
            NappyKind::Both,
            NappyKind::Dry,
        ] {
            assert!(kind.to_api().is_known(), "{kind:?}");
        }
        for colour in [
            Colour::Yellow,
            Colour::Brown,
            Colour::Black,
            Colour::Green,
            Colour::Red,
            Colour::Gray,
        ] {
            assert!(colour.to_api().is_known(), "{colour:?}");
        }
        for texture in [
            Consistency::Solid,
            Consistency::Loose,
            Consistency::Runny,
            Consistency::Mucousy,
            Consistency::Hard,
            Consistency::Pebbles,
            Consistency::Diarrhea,
        ] {
            assert!(texture.to_api().is_known(), "{texture:?}");
        }
    }

    #[test]
    fn the_settings_spellings_read_back_to_the_same_values() {
        assert_eq!(Units::from_setting("oz"), Units::Oz);
        assert_eq!(Units::from_setting("ML"), Units::Ml);
        assert_eq!(Units::Oz.as_str(), "oz");
        assert_eq!(System::from_setting("imperial"), System::Imperial);
        assert_eq!(System::from_setting("anything else"), System::Metric);
    }

    #[test]
    fn the_three_amount_buttons_store_the_three_numbers_the_app_writes() {
        assert_eq!(Amount::Little.to_api().to_stored(), Some(0.0));
        assert_eq!(Amount::Medium.to_api().to_stored(), Some(50.0));
        assert_eq!(Amount::Big.to_api().to_stored(), Some(100.0));
    }
}

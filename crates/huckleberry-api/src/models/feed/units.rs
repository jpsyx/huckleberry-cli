//! The words a feed is described with, and the one conversion that matters.
//!
//! `FeedSide::None` is the odd one: it is a transition marker the app writes
//! while a switch or a resume is in flight, not an absence of a side.

string_enum! {
    /// Which breast.
    FeedSide {
        Left => "left",
        Right => "right",
        /// A transition marker the app writes between sides, not an absence.
        None => "none",
    }
}

string_enum! {
    /// What was in the bottle.
    BottleType {
        BreastMilk => "Breast Milk",
        Formula => "Formula",
        TubeFeeding => "Tube Feeding",
        CowMilk => "Cow Milk",
        GoatMilk => "Goat Milk",
        SoyMilk => "Soy Milk",
        Other => "Other",
    }
}

string_enum! {
    /// How a volume is measured.
    VolumeUnits {
        Millilitres => "ml",
        Ounces => "oz",
    }
}

/// How many millilitres are in a fluid ounce, US customary, which is the one
/// the app means.
pub const MILLILITRES_PER_OUNCE: f64 = 29.573_529_562_5;

impl VolumeUnits {
    /// Converts an amount in these units to millilitres, which is the unit
    /// every total in this crate is kept in.
    #[must_use]
    pub fn to_millilitres(&self, amount: f64) -> f64 {
        match self {
            Self::Ounces => amount * MILLILITRES_PER_OUNCE,
            // An unfamiliar unit is taken at face value rather than guessed
            // at: inventing a conversion factor would be worse than none.
            Self::Millilitres | Self::Unknown(_) => amount,
        }
    }
}

#[cfg(test)]
mod conversions {
    use super::*;

    #[test]
    fn ounces_convert_and_millilitres_do_not() {
        let four_ounces = VolumeUnits::Ounces.to_millilitres(4.0);
        assert!((four_ounces - 118.294_118_25).abs() < 0.001);
        assert!((VolumeUnits::Millilitres.to_millilitres(120.0) - 120.0).abs() < f64::EPSILON);
    }

    #[test]
    fn a_unit_this_crate_does_not_know_is_taken_at_face_value() {
        let unknown = VolumeUnits::Unknown("cc".to_owned());
        assert!((unknown.to_millilitres(30.0) - 30.0).abs() < f64::EPSILON);
    }
}

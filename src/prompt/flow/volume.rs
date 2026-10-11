//! Display units can change without rounding the stored quantity.
use super::{ControlEvent, view::navigation};
use crate::{
    cli::Units,
    prompt::dial::{
        self,
        model::{Dial, DialAction},
    },
    render::format,
    theme::Theme,
};
use crossterm::event::KeyEvent;

/// A retained amount, canonical in millilitres and displayed in chosen units.
pub struct VolumeState {
    millilitres: f64,
    units: Units,
    minimum: i32,
    dial: Dial,
}

impl VolumeState {
    /// Builds the existing two-column dial, retaining unrounded input amounts.
    #[must_use]
    pub fn create_volume(units: Units, amount: Option<f64>, minimum: f64) -> Self {
        let millilitres = amount.map_or(60.0, |amount| format::convert(amount, units, Units::Ml));
        let minimum = i32::from(minimum > 0.0);
        let dial = create_dial(
            units,
            format::convert(millilitres, Units::Ml, units),
            minimum,
        );
        Self {
            millilitres,
            units,
            minimum,
            dial,
        }
    }

    /// Applies a key; changing units preserves quantity until an amount is turned.
    pub fn apply(&mut self, key: KeyEvent) -> ControlEvent {
        if let Some(event) = navigation(key, self.dial.column == 0, true) {
            return event;
        }
        match self.dial.apply(key) {
            DialAction::Submit => ControlEvent::Accept,
            DialAction::Cancel => ControlEvent::Cancel,
            DialAction::Turned(1) => {
                self.units = dial::volume::read_units(&self.dial);
                self.dial = create_dial(self.units, self.value().1, self.minimum).opening_on(1);
                ControlEvent::Stay
            }
            DialAction::Turned(_) => {
                self.millilitres =
                    format::convert(dial::volume::read(&self.dial).1, self.units, Units::Ml);
                ControlEvent::Stay
            }
            _ => ControlEvent::Stay,
        }
    }

    /// The exact quantity in the selected units.
    #[must_use]
    pub fn value(&self) -> (Units, f64) {
        (
            self.units,
            format::convert(self.millilitres, Units::Ml, self.units),
        )
    }

    /// Draws the same volume dial as a standalone prompt.
    #[must_use]
    pub fn render(&self, label: &str, width: u16, theme: Theme) -> Vec<String> {
        dial::draw::render(label, &self.dial, width, theme)
            .into_iter()
            .map(|line| line.replace("Esc back", "Esc home"))
            .collect()
    }
}

fn create_dial(units: Units, amount: f64, minimum: i32) -> Dial {
    Dial::new(vec![
        dial::volume::amounts(units, amount, minimum),
        dial::volume::units_wheel(units),
    ])
}

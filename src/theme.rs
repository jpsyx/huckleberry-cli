//! Semantic colors for the tool's human-facing output.
//!
//! Callers ask for meaning, never for a color: a success message is
//! `theme.success(...)`, a command name is `theme.accent(...)`, a hint is
//! `theme.muted(...)`. Each [`Tone`] maps to one ANSI code, so the palette can
//! be changed in this file alone and nothing else in the codebase ever writes
//! an escape sequence.
//!
//! The palette assumes a dark terminal. Terminals do not reliably say whether
//! they are light or dark, so the safe assumption is the common one, and every
//! tone uses a bright variant that stays legible on a dark background. A light
//! palette later is a second code table plus a branch in [`Theme::active`].
//!
//! Prompt colours follow stderr; `Context::output_theme` gates stdout colours
//! independently. `NO_COLOR` disables both. Redirected streams stay plain.

use std::io::IsTerminal;

/// A semantic role in the tool's output. Color decisions name one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    /// A section title.
    Heading,
    /// A key, a command name, a label.
    Accent,
    /// A value, or text that carries the answer.
    Value,
    /// Secondary text: hints, units, anything the eye may skip.
    Muted,
    /// Something finished and worked.
    Success,
    /// Something worked but deserves a second look.
    Warning,
    /// Something failed.
    Error,
    /// A neutral note about what is happening.
    Info,
    /// An interactive question waiting on an answer.
    Prompt,
    /// The row that is today, among rows that are other days.
    ///
    /// Today is the row a person is looking for, so it is the brightest thing
    /// on the screen rather than the dimmest. The other days stay fully
    /// legible: this brightens one row, it does not dim the rest.
    Today,
    /// A figure sitting inside what is typical for this age.
    Good,
    /// A figure sitting outside it.
    ///
    /// Yellow, and never red. This is a tool a frightened parent opens at 3am,
    /// and a red number is a verdict it is in no position to deliver. The
    /// distinction from [`Tone::Warning`] is the point: that one is for the
    /// tool's own problems, this one is an observation about a number.
    Attention,
    /// A sleep, wherever entries of several kinds are listed together.
    Sleep,
    /// A feed of any sort.
    Feeding,
    /// A diaper or a potty trip.
    Diaper,
    /// A pumping session.
    Pumping,
    /// A milestone.
    Milestone,
}

impl Tone {
    /// The ANSI SGR body for this tone (for example `"96"`, bright cyan).
    #[must_use]
    pub const fn sgr(self) -> &'static str {
        match self {
            Self::Heading => "1;95",
            Self::Value => "97",
            Self::Muted => "90",
            Self::Error => "91",
            Self::Prompt => "1;96",
            // Bold on top of the bright white the other rows already use, so
            // today reads as brighter rather than as a different kind of
            // thing.
            Self::Today => "1;97",
            // Faint, so the block at the foot of a table stays secondary
            // while still carrying its colour. A terminal that ignores SGR 2
            // simply shows the bright colour, which is no worse than before.
            Self::Good => "2;92",
            Self::Attention => "2;93",
            // One hue per tracker, so a stream of forty entries can be read by
            // shape before it is read by word. A kind shares its colour with a
            // role it never appears beside: `Pumping` is the green of
            // `Success` and means nothing of the sort, because in a list of
            // kinds a colour is a category and not a verdict, and every row
            // says its kind in words as well.
            Self::Success | Self::Pumping => "92",
            Self::Warning | Self::Milestone => "93",
            Self::Info | Self::Sleep => "94",
            Self::Diaper => "95",
            Self::Accent | Self::Feeding => "96",
        }
    }

    /// The 4-bit palette index for this tone, for callers that need a color
    /// value rather than an escape sequence (a renderer that paints with color
    /// values takes these). Always in the bright half, `8..=15`.
    #[must_use]
    pub const fn ansi_index(self) -> u8 {
        match self {
            Self::Muted => 8,
            Self::Error => 9,
            // Green means the same thing to a renderer that paints with
            // colour values; the two roles differ in what they are *for*, and
            // in whether they are drawn faint.
            Self::Success | Self::Good | Self::Pumping => 10,
            Self::Warning | Self::Attention | Self::Milestone => 11,
            Self::Info | Self::Sleep => 12,
            Self::Heading | Self::Diaper => 13,
            Self::Accent | Self::Prompt | Self::Feeding => 14,
            Self::Value | Self::Today => 15,
        }
    }
}

/// The palette in use, plus whether color is emitted at all.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    color: bool,
}

impl Theme {
    /// The dark-terminal theme. `color` gates emission, so the same theme
    /// yields plain text when the output is piped or `NO_COLOR` is set.
    #[must_use]
    pub const fn dark(color: bool) -> Self {
        Self { color }
    }

    /// The active theme, with color auto-detected.
    #[must_use]
    pub fn active() -> Self {
        Self::dark(color_enabled())
    }

    /// Wraps `text` in the tone's escape sequence, or returns it unchanged
    /// when color is off.
    #[must_use]
    pub fn paint(self, tone: Tone, text: &str) -> String {
        if self.color {
            format!("\x1b[{}m{text}\x1b[0m", tone.sgr())
        } else {
            text.to_owned()
        }
    }

    /// A section title.
    #[must_use]
    pub fn heading(self, text: &str) -> String {
        self.paint(Tone::Heading, text)
    }

    /// A key, a command name, a label.
    #[must_use]
    pub fn accent(self, text: &str) -> String {
        self.paint(Tone::Accent, text)
    }

    /// A value, or text that carries the answer.
    #[must_use]
    pub fn value(self, text: &str) -> String {
        self.paint(Tone::Value, text)
    }

    /// Secondary text: hints, units, anything the eye may skip.
    #[must_use]
    pub fn muted(self, text: &str) -> String {
        self.paint(Tone::Muted, text)
    }

    /// Something finished and worked.
    #[must_use]
    pub fn success(self, text: &str) -> String {
        self.paint(Tone::Success, text)
    }

    /// Something worked but deserves a second look.
    #[must_use]
    pub fn warning(self, text: &str) -> String {
        self.paint(Tone::Warning, text)
    }

    /// Something failed.
    #[must_use]
    pub fn error(self, text: &str) -> String {
        self.paint(Tone::Error, text)
    }

    /// A neutral note about what is happening.
    #[must_use]
    pub fn info(self, text: &str) -> String {
        self.paint(Tone::Info, text)
    }

    /// An interactive question waiting on an answer.
    #[must_use]
    pub fn prompt(self, text: &str) -> String {
        self.paint(Tone::Prompt, text)
    }

    /// The row that is today.
    #[must_use]
    pub fn today(self, text: &str) -> String {
        self.paint(Tone::Today, text)
    }

    /// A figure inside what is typical for this age.
    #[must_use]
    pub fn good(self, text: &str) -> String {
        self.paint(Tone::Good, text)
    }

    /// A figure outside it. Yellow, never red.
    #[must_use]
    pub fn attention(self, text: &str) -> String {
        self.paint(Tone::Attention, text)
    }

    /// One labelled failure line, as the binary prints it.
    #[must_use]
    pub fn error_line(self, label: &str, message: &str) -> String {
        format!("{} {}", self.error(label), self.value(message))
    }
}

/// Whether stderr can emit colour and `NO_COLOR` is unset.
///
/// Stdout uses its own terminal check in `Context::output_theme`.
#[must_use]
pub fn color_enabled() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_TONE: [Tone; 17] = [
        Tone::Heading,
        Tone::Accent,
        Tone::Value,
        Tone::Muted,
        Tone::Success,
        Tone::Warning,
        Tone::Error,
        Tone::Info,
        Tone::Prompt,
        Tone::Today,
        Tone::Good,
        Tone::Attention,
        Tone::Sleep,
        Tone::Feeding,
        Tone::Diaper,
        Tone::Pumping,
        Tone::Milestone,
    ];

    /// The tone every kind of entry is drawn in, wherever they are listed
    /// together.
    const EVERY_KIND: [Tone; 5] = [
        Tone::Sleep,
        Tone::Feeding,
        Tone::Diaper,
        Tone::Pumping,
        Tone::Milestone,
    ];

    #[test]
    fn color_off_yields_plain_text() {
        let theme = Theme::dark(false);
        for tone in EVERY_TONE {
            assert_eq!(theme.paint(tone, "hello"), "hello");
        }
    }

    #[test]
    fn color_on_wraps_the_text_in_the_tones_code() {
        let theme = Theme::dark(true);
        assert_eq!(theme.accent("hi"), "\x1b[96mhi\x1b[0m");
        assert_eq!(theme.heading("hi"), "\x1b[1;95mhi\x1b[0m");
    }

    #[test]
    fn every_tone_is_legible_on_a_dark_background() {
        for tone in EVERY_TONE {
            let code = tone
                .sgr()
                .rsplit(';')
                .next()
                .expect("an SGR body is never empty");
            let code: u8 = code.parse().expect("an SGR body ends in a number");
            assert!(
                (90..=97).contains(&code),
                "{tone:?} uses a dark foreground: {code}"
            );
            assert!(
                (8..=15).contains(&tone.ansi_index()),
                "{tone:?} leaves the bright half"
            );
        }
    }

    #[test]
    fn the_escape_sequence_and_the_palette_index_agree() {
        for tone in EVERY_TONE {
            let code: u8 = tone.sgr().rsplit(';').next().unwrap().parse().unwrap();
            assert_eq!(
                tone.ansi_index(),
                code - 82,
                "{tone:?} paints two different colors"
            );
        }
    }

    #[test]
    fn every_kind_of_entry_has_a_colour_of_its_own() {
        // A list of forty entries is read by shape first. Two kinds sharing a
        // hue would undo that, which is what this notices.
        for (position, tone) in EVERY_KIND.iter().enumerate() {
            for other in &EVERY_KIND[position + 1..] {
                assert_ne!(
                    tone.ansi_index(),
                    other.ansi_index(),
                    "{tone:?} and {other:?} are the same colour"
                );
            }
        }
    }

    #[test]
    fn a_kind_is_drawn_plainly_so_the_cursor_can_be_the_bright_one() {
        // The row under the cursor is bold white; a kind that was also bold
        // would compete with it.
        for tone in EVERY_KIND {
            assert!(
                !tone.sgr().contains("1;"),
                "{tone:?} is bold, which belongs to the cursor"
            );
            assert_ne!(
                tone.ansi_index(),
                Tone::Today.ansi_index(),
                "{tone:?} is the colour the cursor uses"
            );
        }
    }

    #[test]
    fn today_is_brighter_than_the_days_around_it() {
        // The rows around it are `Value`. Today has to be more than that, not
        // less: it is the row somebody is looking for.
        assert_eq!(Tone::Today.ansi_index(), Tone::Value.ansi_index());
        assert!(
            Tone::Today.sgr().starts_with("1;"),
            "today should be bold on top of the same white: {}",
            Tone::Today.sgr()
        );
        assert!(!Tone::Value.sgr().starts_with("1;"));
    }

    #[test]
    fn a_figure_outside_what_is_typical_is_yellow_and_never_red() {
        assert_eq!(Tone::Attention.ansi_index(), 11, "yellow");
        assert_ne!(
            Tone::Attention.ansi_index(),
            Tone::Error.ansi_index(),
            "a number about a baby is never painted as an error"
        );
    }

    #[test]
    fn the_foot_of_a_table_keeps_its_colour_while_staying_faint() {
        for tone in [Tone::Good, Tone::Attention] {
            assert!(
                tone.sgr().starts_with("2;"),
                "{tone:?} should be faint: {}",
                tone.sgr()
            );
        }
    }

    #[test]
    fn an_error_line_labels_the_message() {
        assert_eq!(
            Theme::dark(false).error_line("error:", "no such file"),
            "error: no such file"
        );
    }
}

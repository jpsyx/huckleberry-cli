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
//! Color is emitted only when stderr is a terminal and `NO_COLOR` is unset:
//! the human channel is stderr, and piping the tool anywhere must yield plain
//! text.

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
}

impl Tone {
    /// The ANSI SGR body for this tone (for example `"96"`, bright cyan).
    #[must_use]
    pub const fn sgr(self) -> &'static str {
        match self {
            Self::Heading => "1;95",
            Self::Accent => "96",
            Self::Value => "97",
            Self::Muted => "90",
            Self::Success => "92",
            Self::Warning => "93",
            Self::Error => "91",
            Self::Info => "94",
            Self::Prompt => "1;96",
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
            Self::Success => 10,
            Self::Warning => 11,
            Self::Info => 12,
            Self::Heading => 13,
            Self::Accent | Self::Prompt => 14,
            Self::Value => 15,
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

    /// One labelled failure line, as the binary prints it.
    #[must_use]
    pub fn error_line(self, label: &str, message: &str) -> String {
        format!("{} {}", self.error(label), self.value(message))
    }
}

/// Whether to emit escape sequences: stderr is a terminal and `NO_COLOR` is
/// unset. Stderr is the human channel (prompts, progress, failures); stdout
/// carries data and is never painted.
#[must_use]
pub fn color_enabled() -> bool {
    std::io::stderr().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVERY_TONE: [Tone; 9] = [
        Tone::Heading,
        Tone::Accent,
        Tone::Value,
        Tone::Muted,
        Tone::Success,
        Tone::Warning,
        Tone::Error,
        Tone::Info,
        Tone::Prompt,
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
    fn an_error_line_labels_the_message() {
        assert_eq!(
            Theme::dark(false).error_line("error:", "no such file"),
            "error: no such file"
        );
    }
}

//! The accent: the one colour that marks what is yours and what is active.
//!
//! An accent is not a preset. It moves three roles — `primary`, `ring` and
//! `message_sent` — and leaves every surface and ink where the preset put
//! them, so it is written to `theme.json` as plain colour overrides and needs
//! no key of its own: the file a user hand-edits and the picker in Settings
//! describe the same thing.
//!
//! Each accent carries a dark and a light pair, chosen against the Classic
//! surfaces so that accent text clears AA on every surface, text on the
//! accent clears AA, and message text on a sent bubble clears 7:1. The tests
//! below hold every pair to that, against every preset of its brightness.

use super::palette::{Palette, Rgb};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accent {
    /// The messenger green. The default, and what every chat looks like on
    /// the phone.
    Green,
    /// Velo's own violet, from the logo.
    Violet,
    /// The logo's ultramarine.
    Blue,
    /// The logo's coral.
    Coral,
}

/// One accent at one brightness.
struct Pair {
    primary: u32,
    sent: u32,
}

impl Accent {
    pub const ALL: [Self; 4] = [Self::Green, Self::Violet, Self::Blue, Self::Coral];

    pub fn id(self) -> &'static str {
        match self {
            Self::Green => "green",
            Self::Violet => "violet",
            Self::Blue => "blue",
            Self::Coral => "coral",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Green => "Green",
            Self::Violet => "Velo violet",
            Self::Blue => "Ultramarine",
            Self::Coral => "Coral",
        }
    }

    fn pair(self, dark: bool) -> Pair {
        let (primary, sent) = match (self, dark) {
            (Self::Green, true) => (0x21c063, 0x144d37),
            (Self::Green, false) => (0x017561, 0xd9fdd3),
            (Self::Violet, true) => (0xa48bff, 0x3b2d7a),
            (Self::Violet, false) => (0x5b3fd9, 0xe8e2ff),
            (Self::Blue, true) => (0x7d90ff, 0x1d2f78),
            (Self::Blue, false) => (0x2f45d8, 0xdde4ff),
            (Self::Coral, true) => (0xff8aa5, 0x6a2438),
            (Self::Coral, false) => (0xc2185b, 0xffe0e9),
        };
        Pair { primary, sent }
    }

    /// The accent's colour itself, for a swatch.
    pub fn swatch(self, dark: bool) -> Rgb {
        Rgb::new(self.pair(dark).primary)
    }

    /// Paint `palette` with this accent.
    pub fn apply(self, palette: &mut Palette, dark: bool) {
        let pair = self.pair(dark);
        palette.primary = Rgb::new(pair.primary);
        palette.ring = Rgb::new(pair.primary);
        palette.message_sent = Rgb::new(pair.sent);
    }

    /// Which accent `palette` is wearing, if it is one of these at all. A
    /// preset with an accent of its own, or a hand-edited colour, is none.
    pub fn of(palette: &Palette, dark: bool) -> Option<Self> {
        Self::ALL.into_iter().find(|accent| {
            let pair = accent.pair(dark);
            palette.primary == Rgb::new(pair.primary) && palette.message_sent == Rgb::new(pair.sent)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Preset;

    fn contrast(a: Rgb, b: Rgb) -> f32 {
        let (x, y) = (a.luminance(), b.luminance());
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn every_accent_is_legible_on_the_classic_presets() {
        for preset in [Preset::ClassicDark, Preset::ClassicLight] {
            let dark = preset.is_dark();
            for accent in Accent::ALL {
                let mut p = preset.palette();
                accent.apply(&mut p, dark);
                for surface in [p.background, p.sidebar, p.secondary, p.elevated] {
                    let ratio = contrast(p.primary, surface);
                    assert!(
                        ratio >= 4.5,
                        "{} on {}: accent text {ratio:.2}:1",
                        accent.id(),
                        preset.id()
                    );
                }
                let ink = contrast(p.on(p.primary), p.primary);
                assert!(
                    ink >= 4.5,
                    "{} on {}: ink {ink:.2}:1",
                    accent.id(),
                    preset.id()
                );
                let bubble = contrast(p.foreground, p.message_sent);
                assert!(
                    bubble >= 7.0,
                    "{} on {}: bubble text {bubble:.2}:1",
                    accent.id(),
                    preset.id()
                );
            }
        }
    }

    #[test]
    fn an_applied_accent_is_recognised_and_green_is_the_classic_default() {
        for preset in [Preset::ClassicDark, Preset::ClassicLight] {
            let dark = preset.is_dark();
            assert_eq!(Accent::of(&preset.palette(), dark), Some(Accent::Green));
            for accent in Accent::ALL {
                let mut p = preset.palette();
                accent.apply(&mut p, dark);
                assert_eq!(Accent::of(&p, dark), Some(accent));
            }
        }
    }
}

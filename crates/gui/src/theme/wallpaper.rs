//! Chat wallpapers: what the conversation is drawn on.
//!
//! Six, each in a light and a dark form chosen by the active preset. All of
//! them are gradients drawn by the GPU — no image files, nothing decoded,
//! nothing to keep in memory — so switching is instant and an idle window
//! costs the same as a plain one. They are this project's own designs: none
//! reproduces another app's artwork.
//!
//! Colours live here rather than in the palette for the reason the palette
//! gives for its speaker ramp: a wallpaper is a *set* of stops chosen to sit
//! behind both bubble colours, and handing its stops out one key at a time in
//! `theme.json` would be a way to break that set one key at a time. The file
//! chooses *which* wallpaper; this module owns what each one looks like.

use std::fmt;
use std::str::FromStr;

use gpui::{Div, Hsla, ParentElement as _, Styled as _, div, linear_color_stop, linear_gradient};
use serde::{Deserialize, Serialize};

use super::palette::{Palette, Rgb};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Wallpaper {
    /// The familiar chat backdrop colour: a deep blue-black in the dark
    /// theme, warm beige in the light one. Solid, so messages read first.
    #[default]
    Classic,
    /// A soft glow from the top corner in the theme's accent: present,
    /// never busy.
    Mist,
    /// The theme's background and nothing else.
    Plain,
    /// Indigo fading into the ground.
    Dusk,
    /// Deep teal.
    Ocean,
    /// Pine green.
    Forest,
    /// A warm ember brown.
    Ember,
}

impl Wallpaper {
    /// In the order Settings offers them.
    pub const ALL: [Self; 7] = [
        Self::Classic,
        Self::Mist,
        Self::Plain,
        Self::Dusk,
        Self::Ocean,
        Self::Forest,
        Self::Ember,
    ];

    /// The value written to `theme.json`.
    pub fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Mist => "mist",
            Self::Plain => "plain",
            Self::Dusk => "dusk",
            Self::Ocean => "ocean",
            Self::Forest => "forest",
            Self::Ember => "ember",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic (default)",
            Self::Mist => "Mist",
            Self::Plain => "Plain",
            Self::Dusk => "Dusk",
            Self::Ocean => "Ocean",
            Self::Forest => "Forest",
            Self::Ember => "Ember",
        }
    }

    /// The colour the gradient starts from (its top-left corner) in a dark
    /// or light theme. `None` for the wallpapers that are the ground itself.
    fn tint(self, dark: bool) -> Option<Rgb> {
        let (dark_tint, light_tint) = match self {
            Self::Classic | Self::Mist | Self::Plain => return None,
            Self::Dusk => (0x221a3d, 0xece6fb),
            Self::Ocean => (0x0c2530, 0xdff1f5),
            Self::Forest => (0x112219, 0xe2f1e7),
            Self::Ember => (0x2b1a15, 0xfbebe0),
        };
        Some(Rgb(if dark { dark_tint } else { light_tint }))
    }

    /// Paint this wallpaper behind `content`.
    ///
    /// Returns a full-size box whose background is the wallpaper. The glow
    /// of `Mist` is a second, translucent layer over the ground, which is
    /// why this returns an element rather than a single fill.
    pub fn paint(self, palette: &Palette, dark: bool) -> Div {
        let ground = hsla(palette.background);
        let base = div().size_full().relative();
        match self {
            Self::Plain => base.bg(ground),
            Self::Classic => base.bg(gpui::rgb(if dark { 0x111212 } else { 0xefeae2 })),
            Self::Mist => {
                // The theme's own accent, so the glow follows a theme.json
                // that moves `ring`.
                let glow = hsla(palette.ring).opacity(if dark { 0.16 } else { 0.10 });
                let clear = hsla(palette.ring).opacity(0.0);
                base.bg(ground)
                    .child(div().absolute().inset_0().bg(linear_gradient(
                        155.0,
                        linear_color_stop(glow, 0.0),
                        linear_color_stop(clear, 0.65),
                    )))
            }
            _ => {
                let tint = hsla(self.tint(dark).unwrap_or(palette.background));
                base.bg(linear_gradient(
                    150.0,
                    linear_color_stop(tint, 0.0),
                    linear_color_stop(ground, 1.0),
                ))
            }
        }
    }

    /// A small swatch of this wallpaper for the picker: the same paint, at
    /// whatever size the caller gives the box.
    pub fn swatch(self, palette: &Palette, dark: bool) -> Div {
        self.paint(palette, dark)
    }
}

fn hsla(colour: Rgb) -> Hsla {
    gpui::rgb(colour.0).into()
}

impl FromStr for Wallpaper {
    type Err = UnknownWallpaperError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|w| w.id() == s)
            .ok_or_else(|| UnknownWallpaperError(s.to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownWallpaperError(pub String);

impl fmt::Display for UnknownWallpaperError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown wallpaper \"{}\"; known wallpapers are ", self.0)?;
        for (i, w) in Wallpaper::ALL.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            write!(f, "\"{}\"", w.id())?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::palette::{
        CLASSIC_DARK as ULTRAMARINE_DARK, CLASSIC_LIGHT as ULTRAMARINE_LIGHT,
    };

    #[test]
    fn every_wallpaper_round_trips_through_its_id() {
        for wallpaper in Wallpaper::ALL {
            assert_eq!(wallpaper.id().parse::<Wallpaper>(), Ok(wallpaper));
        }
        assert!("nope".parse::<Wallpaper>().is_err());
    }

    /// A wallpaper sits behind both bubbles, so its tint must stay close to
    /// the ground it fades into: a dark theme's tint is dark, a light one's
    /// light. Otherwise received bubbles, which are near the ground's colour,
    /// would lose their edge against it.
    #[test]
    fn tints_stay_on_the_ground_side_of_the_bubbles() {
        for wallpaper in Wallpaper::ALL {
            if let Some(tint) = wallpaper.tint(true) {
                assert!(
                    tint.luminance() < ULTRAMARINE_DARK.message_received.luminance() * 0.85,
                    "{} dark tint is lighter than the received bubble",
                    wallpaper.id()
                );
            }
            if let Some(tint) = wallpaper.tint(false) {
                assert!(
                    tint.luminance() > ULTRAMARINE_LIGHT.message_sent.luminance() * 0.9,
                    "{} light tint is too dark for the light theme",
                    wallpaper.id()
                );
            }
        }
    }
}

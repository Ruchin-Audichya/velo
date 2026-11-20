//! Chat wallpapers: what the conversation is drawn on.
//!
//! Sixteen, each in a light and a dark form chosen by the active preset. All
//! of them are drawn by the GPU — gradients, glows, and one pattern of dots
//! laid out from the rem — with no image files, nothing decoded and nothing
//! to keep in memory, so switching is instant and an idle window costs the
//! same as a plain one. They are this project's own designs: none
//! reproduces another app's artwork.
//!
//! Colours live here rather than in the palette for the reason the palette
//! gives for its speaker ramp: a wallpaper is a *set* of stops chosen to sit
//! behind both bubble colours, and handing its stops out one key at a time in
//! `theme.json` would be a way to break that set one key at a time. The file
//! chooses *which* wallpaper; this module owns what each one looks like.

use std::fmt;
use std::str::FromStr;

use gpui::{
    Bounds, Div, Hsla, ParentElement as _, Styled as _, canvas, div, linear_color_stop,
    linear_gradient, point, size,
};
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
    /// Teal and violet glows from opposite corners, like a northern sky.
    Aurora,
    /// Apricot and rose glows: the last light of the day.
    Sunset,
    /// A quiet grid of dots over the ground: texture without a picture.
    Dots,
    /// Soft violet.
    Lavender,
    /// Blush pink.
    Rose,
    /// Fresh mint.
    Mint,
    /// Warm sand.
    Sand,
    /// Deep navy.
    Midnight,
    /// Neutral slate grey.
    Graphite,
}

impl Wallpaper {
    /// In the order Settings offers them.
    pub const ALL: [Self; 16] = [
        Self::Classic,
        Self::Mist,
        Self::Plain,
        Self::Dots,
        Self::Aurora,
        Self::Sunset,
        Self::Dusk,
        Self::Ocean,
        Self::Forest,
        Self::Ember,
        Self::Lavender,
        Self::Rose,
        Self::Mint,
        Self::Sand,
        Self::Midnight,
        Self::Graphite,
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
            Self::Aurora => "aurora",
            Self::Sunset => "sunset",
            Self::Dots => "dots",
            Self::Lavender => "lavender",
            Self::Rose => "rose",
            Self::Mint => "mint",
            Self::Sand => "sand",
            Self::Midnight => "midnight",
            Self::Graphite => "graphite",
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
            Self::Aurora => "Aurora",
            Self::Sunset => "Sunset",
            Self::Dots => "Dots",
            Self::Lavender => "Lavender",
            Self::Rose => "Rose",
            Self::Mint => "Mint",
            Self::Sand => "Sand",
            Self::Midnight => "Midnight",
            Self::Graphite => "Graphite",
        }
    }

    /// The colour the gradient starts from (its top-left corner) in a dark
    /// or light theme. `None` for the wallpapers that are the ground itself.
    fn tint(self, dark: bool) -> Option<Rgb> {
        let (dark_tint, light_tint) = match self {
            Self::Classic | Self::Mist | Self::Plain | Self::Aurora | Self::Sunset | Self::Dots => {
                return None;
            }
            Self::Dusk => (0x221a3d, 0xece6fb),
            Self::Ocean => (0x0c2530, 0xdff1f5),
            Self::Forest => (0x112219, 0xe2f1e7),
            Self::Ember => (0x2b1a15, 0xfbebe0),
            Self::Lavender => (0x241d36, 0xf1ebfa),
            Self::Rose => (0x2d1a22, 0xfbe9ee),
            Self::Mint => (0x10261f, 0xe3f5ec),
            Self::Sand => (0x201b13, 0xf6efe2),
            Self::Midnight => (0x0e1730, 0xe7ecf8),
            Self::Graphite => (0x1d1f22, 0xeceef0),
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
            Self::Aurora => glows(base.bg(ground), (0x2dd4bf, 0x8b5cf6), dark),
            Self::Sunset => glows(base.bg(ground), (0xfb923c, 0xec4899), dark),
            Self::Dots => {
                let dot = hsla(palette.foreground).opacity(if dark { 0.07 } else { 0.08 });
                base.bg(ground).child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, (), window, _| {
                            // From the rem, like every other size: the
                            // pattern scales with the window's text.
                            let step = window.rem_size() * 1.5;
                            let radius = window.rem_size() * 0.1;
                            let mut y = bounds.top() + step / 2.0;
                            let mut row = 0;
                            while y < bounds.bottom() {
                                // Every other row offset by half a step:
                                // a woven grid, not graph paper.
                                let mut x =
                                    bounds.left() + if row % 2 == 0 { step / 2.0 } else { step };
                                while x < bounds.right() {
                                    window.paint_quad(
                                        gpui::fill(
                                            Bounds::new(
                                                point(x - radius, y - radius),
                                                size(radius * 2.0, radius * 2.0),
                                            ),
                                            dot,
                                        )
                                        .corner_radii(radius),
                                    );
                                    x += step;
                                }
                                y += step * 0.866;
                                row += 1;
                            }
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                )
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

/// Two soft glows over `base`, one from the top-left corner and one from the
/// bottom-right, each fading to nothing well before the middle so the
/// conversation itself sits on the plain ground.
fn glows(base: Div, (first, second): (u32, u32), dark: bool) -> Div {
    let strength = if dark { 0.20 } else { 0.14 };
    let layer = |colour: u32, angle: f32| {
        let glow: Hsla = gpui::rgb(colour).into();
        div().absolute().inset_0().bg(linear_gradient(
            angle,
            linear_color_stop(glow.opacity(strength), 0.0),
            linear_color_stop(glow.opacity(0.0), 0.6),
        ))
    };
    base.child(layer(first, 135.0)).child(layer(second, 315.0))
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

//! Avatars.
//!
//! A profile picture when WhatsApp gives us one; otherwise the picture every
//! messenger draws for "no picture": a quiet grey circle with a person — or,
//! for a group, two — in silhouette. An initial on a coloured ground was the
//! earlier answer, and it read as a placeholder waiting to load rather than
//! as somebody who simply has no photo.

use std::sync::Arc;

use gpui::{
    AnyElement, App, Hsla, ImageSource, IntoElement, ParentElement, Pixels, RenderOnce,
    SharedString, Styled, Window, div, img,
};
use gpui_component::ActiveTheme as _;
use gpui_component::{Icon, IconName};

use crate::session::MediaCache;
use crate::theme::ActiveProductTheme as _;

/// Where a contact is, as far as the avatar needs to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Online,
    /// Known to be away. Drawn as a hollow marker rather than a coloured dot,
    /// so availability is never carried by colour alone.
    Away,
}

/// A marker in the avatar's lower corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Badge {
    Presence(Presence),
    /// This conversation is a group, not a person.
    Group,
}

/// A profile picture, or the silhouette drawn when there is none.
#[derive(IntoElement)]
pub struct Avatar {
    /// The JID, or any stable identity. Kept for callers that key on it.
    #[allow(dead_code)]
    identity: SharedString,
    is_group: bool,
    size: Pixels,
    badge: Option<Badge>,
    /// The surface the avatar sits on, which the badge's ring has to match to
    /// read as a cut-out rather than a second circle.
    ground: Option<Hsla>,
    picture: Option<ImageSource>,
}

impl Avatar {
    /// An avatar for `identity`, labelled with the first character of `name`.
    pub fn new(identity: impl Into<SharedString>, _name: &str, size: Pixels) -> Self {
        Self {
            identity: identity.into(),
            is_group: false,
            size,
            badge: None,
            ground: None,
            picture: None,
        }
    }

    /// Mark the avatar with the contact's availability.
    pub fn presence(mut self, presence: Option<Presence>) -> Self {
        if let Some(presence) = presence {
            self.badge = Some(Badge::Presence(presence));
        }
        self
    }

    /// Mark the avatar as a group rather than a person.
    pub fn group(mut self, is_group: bool) -> Self {
        self.is_group = is_group;
        if is_group {
            self.badge = Some(Badge::Group);
        }
        self
    }

    /// The surface behind the avatar, so a badge can ring itself in it.
    ///
    /// Without this the ring uses the panel colour, which is wrong on a
    /// selected row and shows as a faint halo.
    pub fn on(mut self, ground: Hsla) -> Self {
        self.ground = Some(ground);
        self
    }

    pub fn picture(mut self, key: Option<String>, _cache: Option<Arc<dyn MediaCache>>) -> Self {
        self.picture = key.and_then(|key| crate::session::get_avatar_image(&key));
        self
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let product = cx.product();
        let palette = product.palette;
        let ground = self.ground.unwrap_or(cx.theme().sidebar);

        // Neutral on purpose: no photo is not an identity, so it gets no hue.
        // Dark: a mid grey disc with a lighter figure. Light: a pale grey
        // disc with a white figure — the two ways "nobody here" is drawn.
        let (disc, figure) = if product.preset.is_dark() {
            (
                palette.elevated.mix(palette.faint_foreground, 0.45),
                palette.subtle_foreground.mix(palette.foreground, 0.35),
            )
        } else {
            (
                palette.secondary.mix(palette.faint_foreground, 0.35),
                palette.background,
            )
        };
        let silhouette = if self.is_group {
            crate::components::ProductIcon::AvatarGroup
        } else {
            crate::components::ProductIcon::AvatarPerson
        };
        // The group marker says "group" on a photo. On the silhouette it
        // would say it twice.
        let badge = self
            .badge
            .filter(|badge| *badge != Badge::Group || self.picture.is_some());

        let badge_size = (self.size * 0.28).max(cx.product().metrics.badge_min());

        div()
            .relative()
            .flex_shrink_0()
            .size(self.size)
            .child({
                let fallback = div()
                    .size(self.size)
                    .rounded_full()
                    .overflow_hidden()
                    .bg(product.hsla(disc))
                    .child(
                        // Full size and clipped by the circle, so the
                        // shoulders run off its bottom edge.
                        Icon::from(silhouette)
                            .size(self.size)
                            .text_color(product.hsla(figure)),
                    );
                match self.picture {
                    Some(url) => div().relative().size(self.size).child(fallback).child(
                        img(url)
                            .absolute()
                            .inset_0()
                            .size(self.size)
                            .rounded_full()
                            .border_1()
                            .border_color(if product.preset.is_dark() {
                                gpui::white().opacity(0.1)
                            } else {
                                gpui::black().opacity(0.1)
                            }),
                    ),
                    None => fallback,
                }
            })
            .children(badge.map(|badge| render_badge(badge, badge_size, ground, cx)))
    }
}

fn render_badge(badge: Badge, size: Pixels, ground: Hsla, cx: &App) -> AnyElement {
    let ringed = div()
        .absolute()
        .bottom_0()
        .right_0()
        .size(size)
        .rounded_full()
        // The ring is the surface behind the avatar, so the badge reads as a
        // hole punched in the circle rather than a sticker on top of it.
        .border_2()
        .border_color(ground)
        .flex()
        .items_center()
        .justify_center();

    match badge {
        Badge::Presence(Presence::Online) => ringed.bg(cx.theme().success).into_any_element(),
        // Away is the same shape without the fill: shape carries the state, so
        // it survives both a colour-blind reader and a custom palette.
        Badge::Presence(Presence::Away) => ringed
            .bg(cx.theme().secondary)
            .border_color(ground)
            .child(
                div()
                    .size(size * 0.4)
                    .rounded_full()
                    .bg(cx.product().hsla(cx.product().palette.muted_foreground)),
            )
            .into_any_element(),
        Badge::Group => ringed
            .bg(cx.theme().secondary)
            .child(
                Icon::new(IconName::User)
                    .size(size * 0.6)
                    .text_color(cx.theme().muted_foreground),
            )
            .into_any_element(),
    }
}

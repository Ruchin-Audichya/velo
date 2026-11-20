//! Reactions, and the hover actions on a bubble.

use std::collections::HashMap;

use gpui::{
    Animation, AnimationExt as _, App, Entity, InteractiveElement as _, IntoElement, ParentElement,
    SharedString, Styled, div, prelude::FluentBuilder as _,
};
use gpui_component::ActiveTheme as _;
use gpui_component::button::ButtonVariants as _;
use gpui_component::{Icon, Sizable as _};

use crate::app::{BubbleIds, WhatsAppApp};
use crate::components::{ProductIcon, parts};
use crate::theme::Metrics;

/// The reaction chips hanging off a bubble's lower edge.
///
/// Outside the bubble rather than in it: a reaction is something other people
/// added to the message, and drawing it inside makes it read as part of what
/// the author wrote. The overlap is what ties it back to its bubble.
pub fn render_reactions(
    reactions: HashMap<String, Vec<String>>,
    is_from_me: bool,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    // Most-reacted first, ties broken by emoji so the order is stable between
    // frames rather than following the hash map.
    let mut sorted: Vec<_> = reactions.into_iter().collect();
    sorted.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    div()
        .flex()
        .gap(metrics.space_xs())
        .mt(-metrics.reaction_overlap())
        .px(metrics.space_md())
        .map(|el| {
            if is_from_me {
                el.justify_end()
            } else {
                el.justify_start()
            }
        })
        .children(sorted.into_iter().map(|(emoji, senders)| {
            let count = senders.len();
            let emoji: SharedString = emoji.into();

            div()
                .flex()
                .items_center()
                .gap(metrics.space_xxs())
                .px(metrics.space_sm())
                .rounded_full()
                .bg(cx.theme().secondary)
                .border_1()
                .border_color(cx.theme().border)
                .child(div().text_size(metrics.text_small()).child(emoji))
                // A lone reaction needs no "1" beside it; the chip is the count.
                .when(count > 1, |el| {
                    el.child(
                        div()
                            .font_features(crate::components::parts::tabular())
                            .text_size(metrics.text_micro())
                            .text_color(cx.theme().muted_foreground)
                            .child(count.to_string()),
                    )
                })
        }))
}

/// The quick-react strip drawn under a bubble while it is open.
///
/// Inline rather than a popup: the timeline already moves rows for content,
/// and a surface positioned against the bubble would need an anchor the
/// virtual list does not give back. One tap sends — or takes back, when it
/// names the reaction already drawn as ours.
///
/// It springs open the way the phone's does: the strip rises in, then each
/// emoji pops up a beat after the one before it, overshooting a little and
/// settling. Every emoji sits in a fixed cell, so only paint moves and the
/// row's height never changes while it plays — the virtual list measures
/// rows, and a row that grew mid-animation would jolt the timeline.
pub fn render_reaction_picker(
    message_id: &str,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let cell = metrics.text_title() * 1.9;
    let strip =
        div()
            .flex()
            .gap(metrics.space_xxs())
            .items_center()
            .px(metrics.space_xs())
            .py(metrics.space_xxs())
            .mt(metrics.space_xs())
            .rounded_full()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .shadow_md()
            .children(WhatsAppApp::QUICK_REACTIONS.into_iter().enumerate().map(
                |(index, emoji)| {
                    let picker_entity = entity.clone();
                    let picker_id = message_id.to_string();
                    let picker_emoji = emoji.to_string();
                    let full = metrics.text_title();
                    let face = div()
                        .id(SharedString::from(format!("quick-react-face-{index}")))
                        .text_size(full)
                        .line_height(gpui::relative(1.0))
                        .child(emoji)
                        .with_animation(
                            SharedString::from(format!("quick-react-pop-{message_id}-{index}")),
                            Animation::new(POP_STEP * index as u32 + POP_EACH),
                            move |el, t| {
                                let local = staggered(t, index);
                                el.text_size(full * spring(local)).opacity(local.min(1.0))
                            },
                        );
                    // A `Button`, not a styled div: reacting is a command,
                    // and the same rule that made the retry control one
                    // applies here — a pointer-only surface is not a route
                    // keyboard users can take.
                    gpui_component::button::Button::new(SharedString::from(format!(
                        "quick-react-{message_id}-{emoji}"
                    )))
                    .ghost()
                    .rounded_full()
                    .size(cell)
                    .p_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    // A hovered emoji leans in, the way a finger resting on
                    // one does on the phone.
                    .hover(move |style| style.text_size(full * 1.3))
                    .child(face)
                    .tooltip(crate::tr!(react_emoji, emoji))
                    .on_click(move |_, window, cx| {
                        picker_entity.update(cx, |app, cx| {
                            app.toggle_reaction(&picker_id, &picker_emoji, window, cx)
                        });
                    })
                },
            ));
    crate::motion::rise_in(
        strip,
        SharedString::from(format!("quick-react-strip-{message_id}")),
        metrics.space_md(),
    )
}

/// How long one emoji takes to pop, and how far behind the previous one it
/// starts: six of them land in about a third of a second.
const POP_EACH: std::time::Duration = std::time::Duration::from_millis(220);
const POP_STEP: std::time::Duration = std::time::Duration::from_millis(28);

/// Where emoji `index` is in its own pop, given `t` through an animation
/// that lasts its delay plus its pop. 0 until its turn, then 0→1.
fn staggered(t: f32, index: usize) -> f32 {
    let delay = POP_STEP.as_secs_f32() * index as f32;
    let total = delay + POP_EACH.as_secs_f32();
    ((t * total - delay) / POP_EACH.as_secs_f32()).clamp(0.0, 1.0)
}

/// An ease-out with a little overshoot: past full size, then back to it.
/// The "back" curve, which settles exactly at 1.
fn spring(t: f32) -> f32 {
    const C1: f32 = 1.70158;
    const C3: f32 = C1 + 1.0;
    let u = t - 1.0;
    1.0 + C3 * u * u * u + C1 * u * u
}

/// React, reply and copy, revealed on hover.
///
/// Hover-only is acceptable here because none of the three is the only route
/// to its command: each is also on the message's context menu, which is what
/// keyboard and assistive-technology users reach. What this replaces is a
/// clipboard button welded into every bubble's timestamp line.
pub fn render_hover_actions(
    ids: &BubbleIds,
    message_id: String,
    content: String,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    _cx: &App,
) -> impl IntoElement + use<> {
    let reply_id = message_id.clone();
    let reply_entity = entity.clone();
    let picker_id = message_id;
    let picker_entity = entity;
    let has_text = !content.is_empty();

    let action = |id: SharedString, icon: Icon, tip: &'static str| {
        parts::icon_button(id, icon, tip, metrics.icon_button()).xsmall()
    };

    div()
        .flex()
        .gap(metrics.space_xxs())
        .items_center()
        .child(
            // Opens the quick-react strip under this bubble, drawn inline
            // rather than as a popup the virtual list cannot anchor. The
            // same emojis are on the message's context menu, which is the
            // route keyboard users take.
            action(
                ids.react.clone(),
                ProductIcon::Smile.into(),
                crate::tr!(React),
            )
            .on_click(move |_, _window, cx| {
                picker_entity.update(cx, |app, cx| app.toggle_reaction_picker(&picker_id, cx));
            }),
        )
        .child(
            action(
                ids.reply.clone(),
                ProductIcon::Reply.into(),
                crate::tr!(Reply),
            )
            .cursor_pointer()
            .on_click(move |_, window, cx| {
                reply_entity.update(cx, |app, cx| app.begin_reply(&reply_id, window, cx));
            }),
        )
        .when(has_text, |el| {
            el.child(gpui_component::clipboard::Clipboard::new(ids.copy.clone()).value(content))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_emoji_waits_its_turn_then_settles_at_full_size() {
        assert_eq!(staggered(0.0, 3), 0.0, "nothing before its turn");
        assert_eq!(staggered(1.0, 0), 1.0);
        assert_eq!(
            staggered(1.0, 5),
            1.0,
            "the last one finishes with the animation"
        );
        assert!(staggered(0.3, 0) > staggered(0.3, 4), "earlier ones lead");
        assert!(
            (spring(1.0) - 1.0).abs() < 1e-6,
            "settles exactly at full size"
        );
        assert_eq!(spring(0.0), 0.0);
        let peak = (0..=100)
            .map(|i| spring(i as f32 / 100.0))
            .fold(0.0, f32::max);
        assert!(
            peak > 1.05 && peak < 1.15,
            "a small overshoot, not a bounce: {peak}"
        );
    }
}

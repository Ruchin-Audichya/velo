//! The window's destinations.
//!
//! Chats, Status, Channels and Communities share the same navigation on both targets.
//!
//! One list of destinations, drawn along whichever axis there is room for: a
//! strip down the side of a window, a bar across the foot of a phone. The
//! axis follows the responsive layout.
//!
//! On a desktop the strip has two widths. Compact is icons on 44-point
//! targets, each with its tooltip; expanded adds the label and the unread
//! count beside each icon. The button at the top of the strip and Ctrl+B
//! switch between them, and the choice is remembered in the theme file.

use gpui::prelude::*;
use gpui::{
    App, Entity, InteractiveElement, IntoElement, ParentElement, SharedString, Styled, div,
};
use gpui_component::ActiveTheme as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Icon, IconName, Selectable as _};

use crate::app::{Destination, WhatsAppApp};
use crate::components::parts;
use crate::responsive::ResponsiveLayout;
use crate::theme::Metrics;

/// A full-window screen the rail opens over the destinations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailScreen {
    Calls,
    Files,
}

/// The unread label a badge shows: the count, or "99+" past ninety-nine.
#[must_use]
pub fn badge_label(count: usize) -> String {
    if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

/// How one rail item is drawn: the facts the two widths share.
struct Item {
    id: SharedString,
    icon: Icon,
    label: &'static str,
    selected: bool,
    badge: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn render_nav_rail(
    current: Destination,
    // Calls or Files when one of them is on screen. Then it, and not the
    // destination underneath, is what the rail shows as selected.
    screen: Option<RailScreen>,
    unread: [usize; 4],
    // Who is signed in, drawn as the photo at the foot of the strip — the
    // way into your profile and Settings, as in the desktop messengers.
    account: Option<crate::components::AccountSummary>,
    // A newer Velo, when one is out: drawn above Settings until installed.
    update: Option<String>,
    entity: Entity<WhatsAppApp>,
    layout: ResponsiveLayout,
    cx: &App,
) -> impl IntoElement + use<> {
    let metrics = *layout.metrics();
    let update_entity = entity.clone();
    let is_mobile = layout.is_mobile();
    let expanded = layout.rail_expanded();
    // A phone's tab bar is sized by the finger; a desktop strip by its own
    // rail target, roomier than a header icon because it is how every screen
    // is reached.
    let gap = metrics.space_xs();
    let target = if is_mobile {
        let count = (Destination::ALL.len() + 3) as f32;
        metrics
            .touch_target()
            .min((layout.viewport().width - gap * (count - 1.0)) / count)
    } else {
        metrics.rail_target()
    };
    let thickness = if is_mobile {
        target + metrics.space_lg()
    } else {
        layout.rail_width()
    };

    let base = div()
        .id("navigation-rail")
        .tab_group()
        .flex_shrink_0()
        .flex()
        .gap(gap)
        .bg(cx.theme().sidebar)
        .border_color(cx.theme().border);

    let base = if is_mobile {
        // A phone's tab bar spans the foot, so its items spread across it.
        base.w_full()
            .h(thickness)
            .flex_row()
            .items_center()
            .justify_center()
            .border_t_1()
    } else {
        // A strip beside a list starts where the list starts. Centred down the
        // window, icons float in the middle of nothing and line up with no
        // part of what they switch between.
        base.h_full()
            .w(thickness)
            .overflow_y_scroll()
            .flex_col()
            .when(expanded, |el| el.items_stretch().px(metrics.space_sm()))
            .when(!expanded, |el| el.items_center())
            .justify_start()
            .pt(metrics.space_md())
            .border_r_1()
    };

    let toggle_entity = entity.clone();
    let files_entity = entity.clone();
    let calls_entity = entity.clone();
    let settings_entity = entity.clone();
    let profile_entity = entity.clone();

    let destinations = Destination::ALL
        .into_iter()
        .zip(unread)
        .map(|(destination, badge)| {
            let entity = entity.clone();
            render_item(
                Item {
                    id: destination.id().into(),
                    icon: destination_icon(destination),
                    label: destination.label(),
                    selected: screen.is_none() && destination == current,
                    badge,
                },
                target,
                expanded,
                metrics,
                cx,
                move |window, cx| {
                    entity.update(cx, |app, cx| app.set_destination(destination, window, cx));
                },
            )
        })
        .collect::<Vec<_>>();

    base
        // Expand or collapse, at the head of the strip.
        .when(!is_mobile, |el| {
            el.child(
                div()
                    .flex()
                    .when(expanded, |row| row.justify_end())
                    .when(!expanded, |row| row.justify_center())
                    .pb(metrics.space_sm())
                    .child(
                        Button::new("nav-expand")
                            .ghost()
                            .icon(if expanded {
                                IconName::PanelLeftClose
                            } else {
                                IconName::PanelLeftOpen
                            })
                            .tooltip(if expanded {
                                "Collapse sidebar (Ctrl+B)"
                            } else {
                                "Expand sidebar (Ctrl+B)"
                            })
                            .accessibility_label("Toggle sidebar labels")
                            .size(metrics.icon_button())
                            .on_click(move |_, _, cx| {
                                toggle_entity.update(cx, |app, cx| app.toggle_rail(cx));
                            }),
                    ),
            )
        })
        .children(destinations)
        // A hairline between where conversations are and the screens beside them.
        .when(!is_mobile, |el| {
            el.child(
                div()
                    .my(metrics.space_sm())
                    .h(metrics.hairline())
                    .when(expanded, |rule| rule.w_full())
                    .when(!expanded, |rule| rule.w(target * 0.6))
                    .bg(cx.theme().border),
            )
        })
        .child(render_item(
            Item {
                id: "nav-calls".into(),
                icon: crate::components::ProductIcon::Phone.into(),
                label: "Calls",
                selected: screen == Some(RailScreen::Calls),
                badge: 0,
            },
            target,
            expanded,
            metrics,
            cx,
            move |_, cx| calls_entity.update(cx, |app, cx| app.open_call_history(cx)),
        ))
        .child(render_item(
            Item {
                id: "nav-files".into(),
                icon: crate::components::ProductIcon::FileText.into(),
                label: "Files",
                selected: screen == Some(RailScreen::Files),
                badge: 0,
            },
            target,
            expanded,
            metrics,
            cx,
            move |_, cx| {
                files_entity.update(cx, |app, cx| {
                    // One screen at a time: Files replaces Calls.
                    app.calls_open = false;
                    app.files_open = true;
                    if let Some(view) = &app.files_view {
                        view.update(cx, |view, cx| view.refresh(cx));
                    }
                    cx.notify();
                });
            },
        ))
        // Settings and your photo sit at the foot of the strip, apart from the
        // places you go between: they are about you, not about a conversation.
        .when(!is_mobile, |el| el.child(div().flex_1()))
        .children(update.map(|version| {
            crate::motion::pop_in(
                div().child(render_item(
                    Item {
                        id: "nav-update".into(),
                        icon: IconName::ArrowDown.into(),
                        label: "Update",
                        selected: true,
                        badge: 0,
                    },
                    target,
                    expanded,
                    metrics,
                    cx,
                    move |_, cx| update_entity.update(cx, |app, cx| app.install_update(cx)),
                )),
                gpui::ElementId::Name(format!("nav-update-{version}").into()),
            )
        }))
        .child(render_item(
            Item {
                id: "nav-settings".into(),
                icon: IconName::Settings.into(),
                label: "Settings",
                selected: false,
                badge: 0,
            },
            target,
            expanded,
            metrics,
            cx,
            move |window, cx| settings_entity.update(cx, |app, cx| app.open_settings(window, cx)),
        ))
        .children(account.map(|account| {
            // Nearly the whole target, on a rounded tile of its own: at the
            // old size the photo read as an icon among icons, not as you.
            let photo =
                crate::components::Avatar::new(account.name.clone(), &account.name, target * 0.82)
                    .picture(account.avatar_key.clone(), None)
                    .on(cx.theme().secondary);
            div()
                .id("nav-profile")
                .flex()
                .items_center()
                .gap(metrics.space_md())
                .p(metrics.space_xxs())
                .rounded(metrics.radius_lg())
                .bg(cx.theme().secondary)
                .cursor_pointer()
                .hover(|style| style.bg(cx.theme().list_hover))
                .when(!expanded, |el| el.justify_center().size(target))
                .child(photo)
                .when(expanded, |el| {
                    el.child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                parts::one_line()
                                    .text_size(metrics.text_body())
                                    .text_color(cx.theme().foreground)
                                    .child(account.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(metrics.text_micro())
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Profile & account"),
                            ),
                    )
                })
                .on_click(move |_, window, cx| {
                    profile_entity.update(cx, |app, cx| {
                        app.open_settings(window, cx);
                        app.set_settings_section(crate::app::SettingsSection::Account, cx);
                    })
                })
        }))
        .when(!is_mobile, |el| el.pb(metrics.space_lg()))
}

fn destination_icon(destination: Destination) -> Icon {
    match destination {
        Destination::Chats => crate::components::ProductIcon::MessageSquare.into(),
        Destination::Status => crate::components::ProductIcon::CircleDashed.into(),
        Destination::Channels => crate::components::ProductIcon::Channels.into(),
        Destination::Communities => crate::components::ProductIcon::Users.into(),
    }
}

/// One rail item: a destination, Calls, Files or Settings.
///
/// A `Button`, because these need keyboard activation and the theme's focus
/// ring; it is the only route to each screen. The selected one carries a
/// short accent bar at its left edge as well as the selected fill, so where
/// you are reads at a glance even in peripheral vision.
fn render_item(
    item: Item,
    target: gpui::Pixels,
    expanded: bool,
    metrics: Metrics,
    cx: &App,
    on_click: impl Fn(&mut gpui::Window, &mut App) + 'static,
) -> gpui::AnyElement {
    let Item {
        id,
        icon,
        label,
        selected,
        badge,
    } = item;
    let ink = if selected {
        cx.theme().foreground
    } else {
        parts::subtle(cx)
    };
    let badge_id = id.clone();
    let mut button = Button::new(id.clone())
        .ghost()
        .selected(selected)
        .toggled(selected)
        .accessibility_id(id.clone())
        .accessibility_label(label)
        .debug_selector({
            let id = id.clone();
            move || id.to_string()
        })
        .relative()
        .h(target)
        .flex()
        .items_center()
        .rounded(metrics.radius_lg())
        .when(selected, |el| el.bg(cx.theme().list_active))
        .cursor_pointer()
        .on_click(move |_, window, cx| on_click(window, cx))
        .child(icon.size(metrics.rail_icon()).text_color(ink));
    if expanded {
        button = button
            .w_full()
            .justify_start()
            .gap(metrics.space_md())
            .px(metrics.space_md())
            .child(
                div()
                    .flex_1()
                    .text_size(metrics.text_body())
                    .text_color(ink)
                    .when(selected, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                    .child(label),
            )
            .when(badge > 0, |el| {
                el.child(count_pill(&badge_id, badge, metrics, cx))
            });
    } else {
        button = button.w(target).justify_center().tooltip(label);
    }

    div()
        .relative()
        .flex_shrink_0()
        .when(!expanded, |el| el.size(target))
        .child(button)
        // The accent bar, flush with the strip's edge.
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .left(-metrics.space_xs())
                    .top(target * 0.3)
                    .h(target * 0.4)
                    .w(metrics.space_xxs())
                    .rounded_full()
                    .bg(cx.theme().primary),
            )
        })
        // Compact: the count rides the icon's corner, inside the target so
        // it is never clipped and never covers the next item.
        .when(!expanded && badge > 0, |el| {
            el.child(
                div()
                    .absolute()
                    .top(metrics.space_xxs())
                    .right(metrics.space_xxs())
                    .child(count_pill(&badge_id, badge, metrics, cx)),
            )
        })
        .into_any_element()
}

/// An unread count: a pill ringed in the strip's colour so it stands off the
/// icon it overlaps, wide enough for "99+".
fn count_pill(
    id: &SharedString,
    count: usize,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let id = id.to_string();
    div()
        .id(SharedString::from(format!("nav-badge-{id}")))
        .debug_selector(move || format!("nav-badge-{id}"))
        .whitespace_nowrap()
        .min_w(metrics.space_lg() * 1.25)
        .h(metrics.space_lg() * 1.25)
        .px(metrics.space_xs())
        .line_height(gpui::relative(1.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .border_2()
        .border_color(cx.theme().sidebar)
        .bg(cx.theme().primary)
        .font_features(crate::components::parts::tabular())
        .text_size(metrics.text_micro())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(cx.theme().primary_foreground)
        .child(
            div()
                .id("nav-badge-label")
                .debug_selector(|| "nav-badge-label".into())
                .child(badge_label(count)),
        )
}

#[cfg(test)]
mod tests {
    use super::badge_label;

    #[test]
    fn counts_past_ninety_nine_read_99_plus() {
        assert_eq!(badge_label(1), "1");
        assert_eq!(badge_label(99), "99");
        assert_eq!(badge_label(100), "99+");
        assert_eq!(badge_label(12_345), "99+");
    }
}

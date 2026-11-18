//! The window's destinations.
//!
//! Chats, Status, Channels and Communities share the same navigation on both targets.
//!
//! One list of destinations, drawn along whichever axis there is room for: a
//! strip down the side of a window, a bar across the foot of a phone. The
//! axis follows the responsive layout.

use gpui::prelude::*;
use gpui::{App, Entity, InteractiveElement, IntoElement, ParentElement, Styled, div};
use gpui_component::ActiveTheme as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Icon, Selectable as _};

use crate::app::{Destination, WhatsAppApp};
use crate::components::parts;
use crate::responsive::ResponsiveLayout;
use crate::theme::Metrics;

pub fn render_nav_rail(
    current: Destination,
    unread: [usize; 4],
    // Who is signed in, drawn as the photo at the foot of the strip — the
    // way into your profile and Settings, as in the desktop messengers.
    account: Option<crate::components::AccountSummary>,
    entity: Entity<WhatsAppApp>,
    layout: ResponsiveLayout,
    cx: &App,
) -> impl IntoElement + use<> {
    let metrics = *layout.metrics();
    let is_mobile = layout.is_mobile();
    // A phone's tab bar is sized by the finger; a desktop strip is sized by
    // the icon it holds. Using the touch target at every width put a 60px rail
    // beside 34px header icons — two control scales in one window, and the
    // phone's minimum applied where nothing else in the app uses it.
    let gap = metrics.space_sm();
    let target = if is_mobile {
        let count = (Destination::ALL.len() + 3) as f32;
        metrics
            .touch_target()
            .min((layout.viewport().width - gap * (count - 1.0)) / count)
    } else {
        metrics.icon_button()
    };
    let thickness = target + metrics.space_lg();

    let base = div()
        .id("navigation-rail")
        .tab_group()
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(gap)
        .bg(cx.theme().sidebar)
        .border_color(cx.theme().border);

    let base = if is_mobile {
        // A phone's tab bar spans the foot, so its items spread across it.
        base.w_full()
            .h(thickness)
            .flex_row()
            .justify_center()
            .border_t_1()
    } else {
        // A strip beside a list starts where the list starts. Centred down the
        // window, two icons float in the middle of nothing and line up with
        // no part of what they switch between.
        base.h_full()
            .w(thickness)
            .overflow_y_scroll()
            .flex_col()
            .justify_start()
            .pt(metrics.space_lg())
            .border_r_1()
    };

    let files_entity = entity.clone();
    let calls_entity = entity.clone();
    let settings_entity = entity.clone();
    let profile_entity = entity.clone();
    base.children(
        Destination::ALL
            .into_iter()
            .zip(unread)
            .map(|(destination, badge)| {
                render_destination(
                    destination,
                    destination == current,
                    badge,
                    target,
                    entity.clone(),
                    metrics,
                    cx,
                )
            }),
    )
    .child(
        Button::new("nav-calls")
            .ghost()
            .icon(crate::components::ProductIcon::Phone)
            .tooltip("Calls")
            .accessibility_label("Calls")
            .size(target)
            .on_click(move |_, _, cx| calls_entity.update(cx, |app, cx| app.open_call_history(cx))),
    )
    .child(
        Button::new("nav-files")
            .ghost()
            .icon(crate::components::ProductIcon::FileText)
            .tooltip("Files & downloads")
            .accessibility_label("Files and downloads")
            .size(target)
            .on_click(move |_, _, cx| {
                files_entity.update(cx, |app, cx| {
                    app.files_open = true;
                    if let Some(view) = &app.files_view {
                        view.update(cx, |view, cx| view.refresh(cx));
                    }
                    cx.notify();
                });
            }),
    )
    // Settings and your photo sit at the foot of the strip, apart from the
    // places you go between: they are about you, not about a conversation.
    .when(!is_mobile, |el| el.child(div().flex_1()))
    .child(
        Button::new("nav-settings")
            .ghost()
            .icon(gpui_component::IconName::Settings)
            .tooltip("Settings")
            .accessibility_label("Settings")
            .size(target)
            .on_click(move |_, window, cx| {
                settings_entity.update(cx, |app, cx| app.open_settings(window, cx))
            }),
    )
    .children(account.map(|account| {
        Button::new("nav-profile")
            .ghost()
            .size(target)
            .tooltip("Profile")
            .accessibility_label("Profile")
            .child(
                crate::components::Avatar::new(
                    account.name.clone(),
                    &account.name,
                    metrics.icon_button() * 0.8,
                )
                .picture(account.avatar_key.clone(), None)
                .on(cx.theme().sidebar),
            )
            .on_click(move |_, window, cx| {
                profile_entity.update(cx, |app, cx| {
                    app.open_settings(window, cx);
                    app.set_settings_section(crate::app::SettingsSection::Account, cx);
                })
            })
    }))
    .when(!is_mobile, |el| el.pb(metrics.space_lg()))
}

/// One destination. It selects where the window is, the way a chat row selects
/// a conversation, and the selected one stays visibly selected — which is what
/// `Button::selected` carries.
#[allow(clippy::too_many_arguments)]
fn render_destination(
    destination: Destination,
    is_current: bool,
    badge: usize,
    // What the rail sized itself for: a finger below the breakpoint, an icon
    // above it.
    target: gpui::Pixels,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let icon: Icon = match destination {
        Destination::Chats => crate::components::ProductIcon::MessageSquare.into(),
        Destination::Status => crate::components::ProductIcon::CircleDashed.into(),
        Destination::Channels => crate::components::ProductIcon::Channels.into(),
        Destination::Communities => crate::components::ProductIcon::Users.into(),
    };

    // A `Button`: destinations need keyboard activation and a focus ring.
    // This is the only route to each list, so a
    // pointer-only rail would put a whole destination out of reach of the
    // keyboard — and the focus ring the theme defines is drawn by the
    // library's controls, not by anything hand-rolled.
    let button = Button::new(destination.id())
        .ghost()
        .selected(is_current)
        .toggled(is_current)
        .accessibility_id(destination.id())
        .accessibility_label(destination.label())
        .debug_selector(move || destination.id().into())
        .relative()
        .size(target)
        .flex()
        .items_center()
        .justify_center()
        .rounded(metrics.radius_md())
        .tooltip(destination.label())
        .when(is_current, |el| el.bg(cx.theme().list_active))
        .cursor_pointer()
        .on_click(move |_, window, cx| {
            entity.update(cx, |app, cx| app.set_destination(destination, window, cx));
        })
        .child(icon.size(metrics.icon()).text_color(if is_current {
            cx.theme().foreground
        } else {
            parts::subtle(cx)
        }));

    // Button clips its internal content to the icon box. Keep the badge beside
    // it so all digits survive on compact desktop buttons too.
    div()
        .relative()
        .size(target)
        .flex_shrink_0()
        .child(button)
        // A count, not a dot: how many conversations are waiting is the thing
        // worth knowing before deciding to look.
        .when(badge > 0, |el| {
            el.child(
                div()
                    .id(gpui::SharedString::from(format!(
                        "nav-badge-{}",
                        destination.id()
                    )))
                    .debug_selector(move || format!("nav-badge-{}", destination.id()))
                    .absolute()
                    .whitespace_nowrap()
                    .top(metrics.space_xs())
                    .right(metrics.space_xs())
                    .min_w(metrics.space_lg())
                    .min_h(metrics.space_lg())
                    .py(metrics.space_xxs())
                    .line_height(gpui::relative(1.0))
                    .px(metrics.space_xxs())
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .bg(cx.theme().primary)
                    .font_features(crate::components::parts::tabular())
                    .text_size(metrics.text_micro())
                    .text_color(cx.theme().primary_foreground)
                    .child(
                        div()
                            .id("nav-badge-label")
                            .debug_selector(|| "nav-badge-label".into())
                            .child(if badge > 99 {
                                "99+".to_string()
                            } else {
                                badge.to_string()
                            }),
                    ),
            )
        })
}

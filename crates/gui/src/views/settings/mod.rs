//! The Settings screen.
//!
//! A screen rather than a dialog: choosing a theme is exploratory, and a modal
//! you have to dismiss to see what it did to the window is the wrong shape for
//! it. Edits apply live and Escape closes it.

mod appearance;
mod devices;
mod panes;

use gpui::{
    App, Context, Entity, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window, div, prelude::FluentBuilder as _,
};
use gpui_component::ActiveTheme as _;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{Icon, IconName, Selectable as _, Sizable as _};

use crate::app::{SettingsSection, WhatsAppApp};
use crate::components::ProductIcon;
use crate::components::parts;
use crate::theme::Metrics;

pub fn render_settings_view(
    app: &mut WhatsAppApp,
    window: &mut Window,
    cx: &mut Context<WhatsAppApp>,
) -> impl IntoElement + use<> {
    // Before anything reads a plugin's tree, exactly as the conversation
    // does it. Settings is where a plugin's own panel lives, so leaving this
    // to the connected view alone meant a text field a plugin published there
    // had nowhere to hold what was typed and drew no box at all — the one
    // screen where it matters most.
    app.sync_plugin_fields(window, cx);

    let layout = app.responsive_layout(window, cx);
    let metrics = *layout.metrics();
    let entity = cx.entity().clone();
    let Some(settings) = app.settings(cx) else {
        return div().into_any_element();
    };
    let section = settings.section;
    let account = app.account_summary();
    // A fixed column beside a pane needs a window with room to spare. On a
    // phone it left the Appearance previews a few characters wide, so the
    // sections become a strip above the pane instead and the screen is one
    // column all the way down.
    let is_mobile = layout.is_mobile();

    div()
        .size_full()
        .flex()
        .bg(cx.theme().background)
        .when(!is_mobile, |el| {
            el.child(render_nav(section, account, entity.clone(), metrics, cx))
        })
        .child(
            parts::detail_stack()
                .child(render_header(
                    section,
                    is_mobile,
                    entity.clone(),
                    metrics,
                    cx,
                ))
                .when(is_mobile, |el| {
                    el.child(render_section_strip(section, entity.clone(), metrics, cx))
                })
                .child(
                    div()
                        .id("settings-body")
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .p(metrics.space_xxl())
                        // Keyed by section, so choosing another fades it in
                        // and a repaint of the same one does not.
                        .child(crate::motion::fade_in(
                            div().w_full().child(match section {
                                SettingsSection::Appearance => {
                                    appearance::render(settings, entity, metrics, cx)
                                        .into_any_element()
                                }
                                other => panes::render(other, app, entity, metrics, cx),
                            }),
                            gpui::SharedString::from(format!("settings-pane-{section:?}")),
                        )),
                ),
        )
        .into_any_element()
}

fn render_nav(
    section: SettingsSection,
    account: Option<crate::components::AccountSummary>,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let close_entity = entity.clone();

    div()
        .w(metrics.settings_nav_width())
        .flex_shrink_0()
        .flex()
        .flex_col()
        .bg(cx.theme().sidebar)
        .border_r_1()
        .border_color(cx.theme().border)
        .child(
            div()
                .h(metrics.sidebar_header_height())
                .flex()
                .items_center()
                .gap(metrics.space_lg())
                .px(metrics.space_lg())
                .child(
                    Button::new("settings-back")
                        .icon(IconName::ArrowLeft)
                        .ghost()
                        .small()
                        .tooltip(crate::tr!(BackToChats))
                        .cursor_pointer()
                        .on_click(move |_, _window, cx| {
                            close_entity.update(cx, |app, cx| {
                                app.close_settings(cx);
                            });
                        }),
                )
                .child(
                    div()
                        .text_size(metrics.text_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(crate::tr!(Settings)),
                ),
        )
        .child(
            div()
                .id("settings-nav-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(metrics.space_xxs())
                .px(metrics.space_md())
                .pb(metrics.space_lg())
                .children(account.map(|account| render_profile(account, metrics, cx)))
                .children(SettingsSection::ALL.into_iter().map(|item| {
                    render_nav_item(item, item == section, entity.clone(), metrics, cx)
                })),
        )
}

/// Who this device is linked to, at the top of Settings: the account's name
/// large, and whether the link is healthy beneath it.
fn render_profile(
    account: crate::components::AccountSummary,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let dot = if account.is_healthy {
        cx.theme().success
    } else {
        cx.theme().warning
    };
    div()
        .flex()
        .items_center()
        .gap(metrics.space_lg())
        .mx(metrics.space_xs())
        .mt(metrics.space_xs())
        .mb(metrics.space_lg())
        .p(metrics.space_lg())
        .rounded(metrics.radius_lg())
        .bg(cx.theme().secondary)
        .child(
            crate::components::Avatar::new(
                account.name.clone(),
                &account.name,
                metrics.avatar_row(),
            )
            .picture(account.avatar_key.clone(), None)
            .on(cx.theme().secondary),
        )
        .child(
            parts::detail_stack()
                .gap(metrics.space_xxs())
                .child(
                    parts::one_line()
                        .text_size(metrics.text_strong())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(account.name),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(metrics.space_sm())
                        .text_size(metrics.text_small())
                        .text_color(cx.theme().muted_foreground)
                        .child(div().size(metrics.space_sm()).rounded_full().bg(dot))
                        .child(account.status),
                ),
        )
}

/// One destination in the side nav.
///
/// A `Button`, because picking a section is a command and a styled `div`
/// carries neither focus nor keyboard activation — which left the whole
/// desktop navigation pointer-only while the phone strip beside it, built from
/// `Button`s, was reachable from the keyboard. The selection bar is drawn as a
/// child rather than through a variant, because it is the same bar the
/// conversation list uses and "where am I" should read the same way in both.
///
/// The row is one child rather than two, because a `Button` centres its own
/// content: `justify_start` on the frame left seven destinations reading down
/// the middle of a column, which is neither where a list of places starts nor
/// where the eye looks for the next one.
///
/// And the bar is a sibling of the button rather than a child of it, for the
/// same reason turned inside out. A button's children go into that content
/// box, which sits inside the button's own padding — so an absolutely
/// positioned child pinned to "the left edge" lands at the left edge of the
/// *text*, drawn straight through the icon it was supposed to sit beside.
fn render_nav_item(
    section: SettingsSection,
    is_selected: bool,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .relative()
        .w_full()
        .child(
            Button::new(SharedString::from(format!("settings-nav-{}", section.id())))
                .ghost()
                .selected(is_selected)
                .w_full()
                .h_auto()
                .py(metrics.space_md())
                // Room for the bar, and the same room on every row, so the
                // labels line up whether or not one is selected.
                .pl(metrics.space_xl())
                .pr(metrics.space_lg())
                .rounded(metrics.radius_md())
                .text_size(metrics.text_secondary())
                .child(
                    div()
                        .w_full()
                        .flex()
                        .items_center()
                        .justify_start()
                        .gap(metrics.space_lg())
                        .child(
                            Icon::new(icon_for(section))
                                .size(metrics.icon())
                                .flex_shrink_0()
                                .text_color(if is_selected {
                                    cx.theme().primary
                                } else {
                                    cx.theme().muted_foreground
                                }),
                        )
                        .child(
                            parts::detail_stack()
                                .gap(metrics.space_xxs())
                                .child(
                                    parts::one_line()
                                        .text_size(metrics.text_body())
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(cx.theme().foreground)
                                        .child(section.label()),
                                )
                                .child(
                                    parts::one_line()
                                        .text_size(metrics.text_small())
                                        .text_color(cx.theme().muted_foreground)
                                        .child(summary_for(section)),
                                ),
                        ),
                )
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |app, cx| app.set_settings_section(section, cx));
                }),
        )
        .when(is_selected, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .top(metrics.space_md())
                    .bottom(metrics.space_md())
                    .w(metrics.selection_bar_width())
                    .rounded_r(metrics.selection_bar_width())
                    .bg(cx.theme().primary),
            )
        })
}

/// The sections as a scrolling strip, for a window with no room beside the
/// pane. Same destinations as the side nav, same selection.
fn render_section_strip(
    section: SettingsSection,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .id("settings-sections")
        .flex_shrink_0()
        .flex()
        .gap(metrics.space_xs())
        .px(metrics.space_lg())
        .py(metrics.space_md())
        .overflow_x_scroll()
        .border_b_1()
        .border_color(cx.theme().border)
        .children(SettingsSection::ALL.into_iter().map(|item| {
            let entity = entity.clone();
            Button::new(SharedString::from(format!("settings-tab-{}", item.id())))
                .label(item.label())
                .ghost()
                .small()
                .selected(item == section)
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |app, cx| app.set_settings_section(item, cx));
                })
        }))
}

fn render_header(
    section: SettingsSection,
    is_mobile: bool,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let back_entity = entity.clone();

    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_between()
        .gap(metrics.space_lg())
        .min_h(metrics.header_height())
        .py(metrics.space_md())
        .px(metrics.space_xxl())
        .border_b_1()
        .border_color(cx.theme().border)
        // With no side nav there is no back button in it either, and Escape
        // is not a key a phone has.
        .when(is_mobile, |el| {
            el.child(
                Button::new("settings-back-mobile")
                    .icon(IconName::ArrowLeft)
                    .flex_shrink_0()
                    .ghost()
                    .small()
                    .tooltip(crate::tr!(BackToChats))
                    .cursor_pointer()
                    .on_click(move |_, _window, cx| {
                        back_entity.update(cx, |app, cx| app.close_settings(cx));
                    }),
            )
        })
        .child(
            div()
                .debug_selector(|| "settings-header-text".into())
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .min_w_0()
                        .text_size(metrics.text_heading())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(section.label()),
                )
                .children(description_for(section).map(|text| {
                    div()
                        .min_w_0()
                        .text_size(metrics.text_small())
                        .text_color(cx.theme().muted_foreground)
                        .child(text)
                })),
        )
        .child(
            Button::new("settings-close")
                .icon(IconName::Close)
                .flex_shrink_0()
                .ghost()
                .small()
                .tooltip(crate::tr!(CloseSettings))
                .cursor_pointer()
                .on_click(move |_, _window, cx| {
                    entity.update(cx, |app, cx| {
                        app.close_settings(cx);
                    });
                }),
        )
}

fn description_for(section: SettingsSection) -> Option<&'static str> {
    match section {
        SettingsSection::Appearance => {
            Some(crate::tr!(PresetsWriteToTheSameThemeJsonYouCanEditByHand))
        }
        SettingsSection::Privacy => Some(crate::tr!(ThisDeviceSIdentityAndHowToStartOver)),
        SettingsSection::Storage => Some(crate::tr!(WhatThisClientKeepsOnDisk)),
        SettingsSection::Plugins => Some(match crate::platform::plugins::home() {
            crate::platform::PluginHome::Folder => {
                crate::tr!(LoadedFromThePluginsFolderEachOneSaysWhatItMayDo)
            }
            crate::platform::PluginHome::Page => {
                crate::tr!(KeptInThisBrowserEachOneSaysWhatItMayDo)
            }
            crate::platform::PluginHome::AnotherTab => {
                crate::tr!(KeptInThisBrowserAndLoadedByTheTabHoldingThisAccount)
            }
        }),
        _ => None,
    }
}

/// The line under each section's name in the side nav: what is inside it,
/// so the right section is found by reading rather than by opening each.
fn summary_for(section: SettingsSection) -> &'static str {
    match section {
        SettingsSection::Account => "Linked device and chat history",
        SettingsSection::Appearance => "Theme, wallpaper, text size, language",
        SettingsSection::Notifications => "Alerts, sounds, start with Windows",
        SettingsSection::AudioVideo => "Microphone, speakers, camera, ringtone",
        SettingsSection::Privacy => "Encryption keys and this device",
        SettingsSection::Storage => "Downloads, cache and disk use",
        SettingsSection::Plugins => "Add-ons and what they may do",
        SettingsSection::Advanced => "Diagnostics and keyboard shortcuts",
    }
}

fn icon_for(section: SettingsSection) -> Icon {
    match section {
        SettingsSection::Account => Icon::new(IconName::CircleUser),
        SettingsSection::Appearance => Icon::new(IconName::Palette),
        SettingsSection::Notifications => Icon::new(IconName::Bell),
        SettingsSection::AudioVideo => Icon::new(ProductIcon::Volume),
        SettingsSection::Privacy => Icon::new(ProductIcon::Shield),
        SettingsSection::Storage => Icon::new(IconName::HardDrive),
        SettingsSection::Plugins => Icon::new(IconName::LayoutDashboard),
        SettingsSection::Advanced => Icon::new(IconName::Settings2),
    }
}

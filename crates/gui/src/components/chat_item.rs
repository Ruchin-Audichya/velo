//! One conversation in the sidebar list.
//!
//! A row is a surface, not a command: it selects a conversation rather than
//! running an action, so it stays a clickable `div` and the selection itself
//! is what carries the state. Everything it draws is decided in
//! [`crate::app::ChatRow`]; this file only lays it out.

use std::sync::Arc;

use gpui::prelude::*;
use gpui::{
    AnyElement, App, Entity, IntoElement, ParentElement, SharedString, StatefulInteractiveElement,
    Styled, div,
};
use gpui_component::ActiveTheme as _;
use gpui_component::menu::ContextMenuExt as _;
use gpui_component::{Icon, IconName};

use crate::app::chat_actions::{
    ArchiveChat, ClearChat, DeleteChat, MUTE_EIGHT_HOURS, MUTE_ONE_WEEK, MarkChatUnread, MuteChat,
    PinChat,
};
use crate::app::{ChatOpen, ChatRow, Preview, PreviewGlyph, Unread, WhatsAppApp};
use crate::components::parts;
use crate::components::{ProductIcon, status_ticks};
use crate::responsive::ResponsiveLayout;
use crate::utils::format_list_time;

use super::Avatar;

pub fn render_chat_item(
    row: ChatRow,
    is_selected: bool,
    entity: Entity<WhatsAppApp>,
    layout: ResponsiveLayout,
    media: Option<Arc<dyn crate::session::MediaCache>>,
    cx: &App,
    // `use<>`: the element reads colours out of the theme but retains nothing
    // borrowed from `cx`, so it must not inherit its lifetime — the virtual
    // list builds rows inside a closure holding `&mut Context`.
) -> impl IntoElement + use<> {
    let metrics = *layout.metrics();
    let jid = row.jid.clone();
    let name: SharedString = row.name.clone().into();
    let has_unread = row.has_unread();
    let toggle_jid = row.jid.clone();
    let toggle_entity = entity.clone();
    let menu_jid: SharedString = row.jid.clone().into();
    let menu_pinned = row.pinned;
    let menu_archived = row.archived;
    let menu_unread = has_unread;
    let menu_muted = row
        .muted_until
        .is_some_and(|until| until > wacore::time::now_utc());

    // The row's own ground, which the avatar's badge rings itself in.
    let ground = if is_selected {
        cx.theme().list_active
    } else {
        cx.theme().sidebar
    };

    div()
        .id(SharedString::from(format!("chat-{jid}")))
        .debug_selector(|| "chat-row".into())
        .w_full()
        .h(layout.chat_item_height())
        .flex()
        .items_center()
        .gap(metrics.space_lg())
        .px(metrics.chat_row_padding_x())
        .rounded(metrics.radius_lg())
        .relative()
        .cursor_pointer()
        .when(is_selected, |el| el.bg(cx.theme().list_active))
        .when(!is_selected, |el| {
            let hover = cx.theme().list_hover;
            el.hover(move |s| s.bg(hover))
        })
        .when(row.tree_depth > 0, |el| {
            el.pl(metrics.chat_row_padding_x() + metrics.space_xl())
        })
        .cursor_pointer()
        .on_click({
            let synthetic = row.synthetic_community;
            move |_, window, cx| {
                entity.update(cx, |this, cx| {
                    if synthetic {
                        this.toggle_community(&jid, cx);
                    } else {
                        this.select_chat(jid.clone(), ChatOpen::ToCompose, window, cx)
                    }
                });
            }
        })
        .when(row.community_toggle_visible, |el| {
            let hover = cx.theme().list_hover;
            el.child(
                div()
                    .id(SharedString::from(format!("community-toggle-{}", row.jid)))
                    .w(metrics.icon_small())
                    .h(metrics.icon_small())
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .rounded(metrics.radius_sm())
                    .hover(move |style| style.bg(hover))
                    .child(
                        Icon::new(if row.community_expanded {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .size(metrics.icon_small())
                        .text_color(parts::subtle(cx)),
                    )
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        toggle_entity.update(cx, |this, cx| this.toggle_community(&toggle_jid, cx));
                    }),
            )
        })
        .child(
            Avatar::new(row.jid.clone(), &row.name, layout.avatar_size())
                .group(row.is_group)
                .picture(row.avatar_key.clone(), media)
                .on(ground),
        )
        .child(
            parts::detail_stack()
                .gap(metrics.space_xs())
                .child(render_name_row(&row, name, has_unread, metrics, cx))
                // A community header is a title, not a conversation: WhatsApp
                // draws its name alone, without an empty preview line.
                .when(!row.synthetic_community, |el| {
                    el.child(render_preview_row(&row, has_unread, metrics, cx))
                }),
        )
        // WhatsApp's chat menu, in its order. Every item is reachable without
        // opening the chat, which is the point of having it on the row.
        .context_menu(move |menu, _window, _cx| {
            let jid = menu_jid.clone();
            let menu = menu
                .menu(
                    if menu_archived {
                        "Unarchive chat"
                    } else {
                        "Archive chat"
                    },
                    Box::new(ArchiveChat {
                        jid: jid.clone(),
                        archive: !menu_archived,
                    }),
                )
                .separator();
            let menu = if menu_muted {
                menu.menu(
                    "Unmute notifications",
                    Box::new(MuteChat {
                        jid: jid.clone(),
                        seconds: Some(0),
                    }),
                )
            } else {
                menu.menu(
                    "Mute for 8 hours",
                    Box::new(MuteChat {
                        jid: jid.clone(),
                        seconds: Some(MUTE_EIGHT_HOURS),
                    }),
                )
                .menu(
                    "Mute for 1 week",
                    Box::new(MuteChat {
                        jid: jid.clone(),
                        seconds: Some(MUTE_ONE_WEEK),
                    }),
                )
                .menu(
                    "Mute always",
                    Box::new(MuteChat {
                        jid: jid.clone(),
                        seconds: None,
                    }),
                )
            };
            menu.separator()
                .menu(
                    if menu_pinned {
                        "Unpin chat"
                    } else {
                        "Pin chat"
                    },
                    Box::new(PinChat {
                        jid: jid.clone(),
                        pin: !menu_pinned,
                    }),
                )
                .menu(
                    if menu_unread {
                        "Mark as read"
                    } else {
                        "Mark as unread"
                    },
                    Box::new(MarkChatUnread {
                        jid: jid.clone(),
                        unread: !menu_unread,
                    }),
                )
                .separator()
                .menu("Clear chat", Box::new(ClearChat { jid: jid.clone() }))
                .menu("Delete chat", Box::new(DeleteChat { jid }))
        })
}

fn render_name_row(
    row: &ChatRow,
    name: SharedString,
    has_unread: bool,
    metrics: crate::theme::Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_baseline()
        .gap(metrics.space_md())
        .min_w_0()
        .child(
            // One line, laid out as a row. It used to be a text box holding
            // three blocks, so the tag and the disambiguator each took a line
            // of their own — the tag's chip stretched into a full-width grey
            // bar, and a hash sat above the name like a heading.
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .items_center()
                .gap(metrics.space_sm())
                .text_size(metrics.text_body())
                .text_color(if row.hierarchy_context {
                    cx.theme().muted_foreground
                } else {
                    cx.theme().foreground
                })
                .font_weight(gpui::FontWeight::NORMAL)
                .child(parts::one_line().min_w_0().child(name))
                .children(row.disambiguator.as_ref().map(|ordinal| {
                    div()
                        .flex_shrink_0()
                        .font_weight(gpui::FontWeight::NORMAL)
                        .text_size(metrics.text_small())
                        .text_color(parts::subtle(cx))
                        .child(format!("#{ordinal}"))
                }))
                .children(row.hierarchy_label.map(|label| {
                    div()
                        .flex_shrink_0()
                        .rounded(metrics.radius_sm())
                        .bg(cx.theme().secondary)
                        .px(metrics.space_xs())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_size(metrics.text_micro())
                        .text_color(cx.theme().secondary_foreground)
                        .child(label)
                })),
        )
        .children(
            row.muted_until
                .filter(|until| *until > wacore::time::now_utc())
                .map(|_| {
                    Icon::new(crate::components::ProductIcon::BellOff)
                        .flex_shrink_0()
                        .size(metrics.icon_small())
                        .text_color(parts::subtle(cx))
                }),
        )
        .children(row.pinned.then(|| {
            Icon::new(crate::components::ProductIcon::Pin)
                .flex_shrink_0()
                .size(metrics.icon_small())
                .text_color(parts::subtle(cx))
        }))
        .children(row.timestamp.map(|timestamp| {
            parts::meta()
                .flex_shrink_0()
                .text_size(metrics.text_meta())
                // The time joins the badge in saying "something is waiting
                // here", so the two agree rather than competing.
                .text_color(if has_unread {
                    cx.theme().primary
                } else {
                    parts::subtle(cx)
                })
                .child(format_list_time(&timestamp))
        }))
}

fn render_preview_row(
    row: &ChatRow,
    has_unread: bool,
    metrics: crate::theme::Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    div()
        .flex()
        .items_center()
        .gap(metrics.space_sm())
        .min_w_0()
        // No kind tag: groups carry a badge on their avatar, and channels
        // and communities are listed under their own section.
        .child(render_preview(&row.preview, row.is_group, metrics, cx))
        .children(render_unread(
            &row.jid,
            &row.unread,
            has_unread,
            metrics,
            cx,
        ))
}

fn render_preview(
    preview: &Preview,
    is_group: bool,
    metrics: crate::theme::Metrics,
    cx: &App,
) -> AnyElement {
    let line = || {
        div()
            .flex()
            .items_center()
            .gap(metrics.space_sm())
            .flex_1()
            .min_w_0()
            // The preview is what a row is read for: at the secondary size it
            // was a squint, and WhatsApp sets it nearly as large as the name.
            .text_size(metrics.text_body() * 0.96)
    };
    let text = |content: String| parts::one_line().flex_1().min_w_0().child(content);

    match preview {
        Preview::Empty => line()
            .text_color(parts::subtle(cx))
            .italic()
            .child(crate::tr!(NoMessages))
            .into_any_element(),

        // Typing is the one preview that is happening right now, so it takes
        // the accent colour the rest of the row spends on unread state.
        Preview::Typing(summary) => crate::motion::breathe(
            line().text_color(cx.theme().primary).child(text(
                crate::i18n::Language::current().typing(summary, is_group, true),
            )),
            "typing-pulse",
        )
        .into_any_element(),

        Preview::Draft(draft) => line()
            .child(
                div()
                    .flex_shrink_0()
                    .text_color(cx.theme().warning)
                    .child(crate::tr!(Draft)),
            )
            .text_color(cx.theme().muted_foreground)
            .child(text(draft.clone()))
            .into_any_element(),

        Preview::Message {
            prefix,
            glyph,
            text: body,
            status,
        } => line()
            .text_color(cx.theme().muted_foreground)
            .children(status.map(|status| status_ticks(status, metrics.icon_small(), cx)))
            // Not `flex_shrink_0`: the prefix is a person's name and comes
            // from the peer, so a row that refuses to shrink it is a row
            // where the preview is squeezed to nothing and the message
            // disappears from the list.
            .children(prefix.as_ref().map(|prefix| {
                parts::one_line()
                    .min_w_0()
                    .text_color(parts::subtle(cx))
                    .child(format!("{prefix}:"))
            }))
            .children(glyph.map(|glyph| {
                Icon::new(icon_for(glyph))
                    .size(metrics.icon_small())
                    .flex_shrink_0()
                    .text_color(parts::subtle(cx))
            }))
            // Already one unstyled line with its markers taken out: the row
            // is built once per change and this runs per visible row per
            // frame, over text with no length limit.
            .child(text(body.clone()))
            .into_any_element(),
    }
}

fn render_unread(
    jid: &str,
    unread: &Unread,
    has_unread: bool,
    metrics: crate::theme::Metrics,
    cx: &App,
) -> Option<AnyElement> {
    if !has_unread {
        return None;
    }
    let badge_id = format!("unread-{jid}");
    let badge = div()
        .id(SharedString::from(badge_id.clone()))
        .debug_selector(move || badge_id.clone())
        .whitespace_nowrap()
        .flex_shrink_0()
        .rounded_full()
        .bg(cx.theme().primary)
        .flex()
        .items_center()
        .justify_center();

    Some(match unread {
        // Keyed by the count, so a new message makes the badge pop once.
        Unread::Count(count) => crate::motion::pop_in(
            badge
                .min_w(metrics.space_xxl())
                .min_h(metrics.space_xxl())
                .py(metrics.space_xxs())
                .line_height(gpui::relative(1.0))
                .px(metrics.space_sm())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .font_features(crate::components::parts::tabular())
                .text_size(metrics.text_meta())
                .text_color(cx.theme().primary_foreground)
                .child(
                    div()
                        .id("unread-label")
                        .debug_selector(|| "unread-label".into())
                        .child(format_count(*count)),
                ),
            gpui::ElementId::Name(format!("unread-pop-{jid}-{count}").into()),
        )
        .into_any_element(),
        // Marked unread by hand has no number. A pill containing a bullet
        // pretends to be a count of something; a dot does not.
        Unread::Marked => badge.size(metrics.space_md()).into_any_element(),
        Unread::None => return None,
    })
}

/// Cap the badge so a long-neglected group cannot widen the row.
fn format_count(count: u32) -> String {
    if count > 999 {
        "999+".to_string()
    } else {
        count.to_string()
    }
}

fn icon_for(glyph: PreviewGlyph) -> ProductIcon {
    match glyph {
        PreviewGlyph::Image => ProductIcon::Image,
        PreviewGlyph::Video => ProductIcon::Film,
        PreviewGlyph::Audio => ProductIcon::Mic,
        PreviewGlyph::Document => ProductIcon::FileText,
        PreviewGlyph::Sticker => ProductIcon::Sticker,
    }
}

#[cfg(test)]
mod tests {
    use super::format_count;

    #[test]
    fn a_neglected_group_cannot_widen_the_row() {
        assert_eq!(format_count(65), "65");
        assert_eq!(format_count(999), "999");
        assert_eq!(format_count(4_312), "999+");
    }
}

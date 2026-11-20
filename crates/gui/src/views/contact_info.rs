//! Contact info / Group info: what clicking a chat's name opens.
//!
//! The same column on the right that Shared files uses, because it is the
//! same kind of thing — a look at the conversation from outside it — and the
//! two are mutually exclusive: one link at the bottom of this panel is the
//! way into the other.

use std::sync::Arc;

use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _};
use velo_core::{Chat, GroupRoster, MediaType};

use crate::app::WhatsAppApp;
use crate::components::{Avatar, ProductIcon};
use crate::theme::Metrics;

/// What a person's address says about them, in words.
///
/// A phone-number JID is shown as the number; a privacy-ID (`@lid`) JID
/// carries no number at all — WhatsApp hides it from linked devices when
/// the person is not a saved contact — and saying so beats printing an
/// opaque id that looks like a phone number and is not one.
pub(crate) fn describe_address(jid: &str) -> (Option<String>, &'static str) {
    let (user, server) = jid.split_once('@').unwrap_or((jid, ""));
    let user = user.split([':', '.']).next().unwrap_or(user);
    match server {
        "s.whatsapp.net" | "c.us" if user.chars().all(|c| c.is_ascii_digit()) => {
            (Some(format_number(user)), "Phone")
        }
        "lid" => (None, "Number hidden by WhatsApp (privacy ID)"),
        "g.us" => (None, "Group"),
        "newsletter" => (None, "Channel"),
        _ => (None, ""),
    }
}

/// `919812345678` → `+91 98123 45678`; anything it cannot place gets `+`
/// and the digits.
fn format_number(digits: &str) -> String {
    if let Some(rest) = digits.strip_prefix("91")
        && rest.len() == 10
    {
        return format!("+91 {} {}", &rest[..5], &rest[5..]);
    }
    if let Some(rest) = digits.strip_prefix('1')
        && rest.len() == 10
    {
        return format!("+1 {} {} {}", &rest[..3], &rest[3..6], &rest[6..]);
    }
    format!("+{digits}")
}

#[derive(Default)]
struct Counts {
    photos: usize,
    videos: usize,
    docs: usize,
    links: usize,
}

fn counts(chat: &Chat) -> Counts {
    let mut counts = Counts::default();
    for message in chat.messages.iter().filter(|m| !m.revoked) {
        match message.media.as_ref().map(|m| &m.media_type) {
            Some(MediaType::Image) => counts.photos += 1,
            Some(MediaType::Video) => counts.videos += 1,
            Some(MediaType::Document) => counts.docs += 1,
            _ => {}
        }
        if message.content.contains("http://") || message.content.contains("https://") {
            counts.links += 1;
        }
    }
    counts
}

fn mute_line(chat: &Chat) -> String {
    match chat.muted_until {
        Some(until) if until == chrono::DateTime::<chrono::Utc>::MAX_UTC => "Muted".into(),
        Some(until) if until > wacore::time::now_utc() => format!(
            "Muted until {}",
            until.with_timezone(&chrono::Local).format("%b %-d, %H:%M")
        ),
        _ => "On".into(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    chat: &Chat,
    roster: Option<&GroupRoster>,
    member_picture: &dyn Fn(&str) -> Option<String>,
    is_own_number: bool,
    can_call: bool,
    entity: Entity<WhatsAppApp>,
    media: Option<Arc<dyn crate::session::MediaCache>>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let name: SharedString = crate::app::chat_row::display_name(&chat.name, is_own_number).into();
    let (number, address_note) = describe_address(&chat.jid);
    let counts = counts(chat);
    let close = entity.clone();
    let files = entity.clone();
    let search = entity.clone();
    let audio = entity.clone();
    let video = entity.clone();
    let audio_jid = chat.jid.clone();
    let video_jid = chat.jid.clone();
    let title = if chat.is_group {
        "Group info"
    } else {
        "Contact info"
    };
    let subtitle: SharedString = if chat.is_group {
        match roster {
            Some(roster) => format!("Group · {} members", roster.size()).into(),
            None => "Group".into(),
        }
    } else {
        number
            .clone()
            .unwrap_or_else(|| address_note.to_string())
            .into()
    };

    let section = |label: &'static str| {
        div()
            .pt(metrics.space_lg())
            .pb(metrics.space_xs())
            .text_size(metrics.text_small())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(label)
    };
    let fact = |label: &'static str, value: SharedString| {
        div()
            .flex()
            .justify_between()
            .gap(metrics.space_md())
            .py(metrics.space_xs())
            .text_size(metrics.text_secondary())
            .child(div().text_color(cx.theme().muted_foreground).child(label))
            .child(
                div()
                    .text_color(cx.theme().foreground)
                    .truncate()
                    .child(value),
            )
    };
    let action = |id: &'static str, icon: Icon, label: &'static str, enabled: bool| {
        Button::new(id)
            .outline()
            .icon(icon)
            .label(label)
            .small()
            .disabled(!enabled)
    };

    let members = roster.map(|roster| {
        let mut members: Vec<_> = roster.members.iter().collect();
        // You last, the way WhatsApp lists a group: the others are who the
        // reader came to look at.
        members.sort_by_key(|m| (m.is_self, m.name.is_none(), m.name.clone()));
        div()
            .flex()
            .flex_col()
            .children(members.into_iter().take(256).map(|member| {
                let label = if member.is_self {
                    "You".to_string()
                } else {
                    member.name.clone().unwrap_or_else(|| {
                        describe_address(&member.jid)
                            .0
                            .unwrap_or_else(|| "Unknown member".into())
                    })
                };
                let person = member.jid.clone();
                let person_name = member.name.clone();
                let dm = entity.clone();
                div()
                    .id(SharedString::from(format!("member-{}", member.jid)))
                    .flex()
                    .items_center()
                    .gap(metrics.space_md())
                    .py(metrics.space_xs())
                    .px(metrics.space_sm())
                    .rounded(metrics.radius_md())
                    // Yourself is not someone to message.
                    .when(!member.is_self, |row| {
                        row.cursor_pointer()
                            .hover(|style| style.bg(cx.theme().secondary))
                            .tooltip({
                                let tip = SharedString::from(
                                    crate::tr!(message_person, label.clone()).to_string(),
                                );
                                move |window, cx| {
                                    gpui_component::tooltip::Tooltip::new(tip.clone())
                                        .build(window, cx)
                                }
                            })
                            .on_click(move |_, window, cx| {
                                let person = person.clone();
                                let person_name = person_name.clone();
                                dm.update(cx, |app, cx| {
                                    app.message_person(person, person_name, window, cx)
                                });
                            })
                    })
                    .child(
                        Avatar::new(member.jid.clone(), &label, metrics.avatar_inline())
                            .picture(member_picture(&member.jid), None)
                            .on(cx.theme().sidebar),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(metrics.text_secondary())
                            .child(label),
                    )
            }))
    });

    div()
        .id("contact-info")
        .w(metrics.details_width())
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().foreground)
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .px(metrics.space_lg())
                .py(metrics.space_md())
                .border_b_1()
                .border_color(cx.theme().border)
                .child(div().text_size(metrics.text_heading()).child(title))
                .child(
                    Button::new("close-contact-info")
                        .ghost()
                        .icon(IconName::Close)
                        .tooltip("Close")
                        .on_click(move |_, _, cx| {
                            close.update(cx, |app, cx| {
                                app.contact_info_open = false;
                                cx.notify();
                            })
                        }),
                ),
        )
        .child(
            div()
                .id("contact-info-body")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px(metrics.space_lg())
                .pb(metrics.space_xl())
                .flex()
                .flex_col()
                // Who: the picture large, the name, the number.
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap(metrics.space_sm())
                        .pt(metrics.space_xl())
                        .child(
                            Avatar::new(chat.jid.clone(), &chat.name, metrics.avatar_call() * 1.5)
                                .group(chat.is_group)
                                .picture(chat.avatar_cache_key.clone(), media)
                                .on(cx.theme().sidebar),
                        )
                        .child(
                            div()
                                .text_size(metrics.text_title())
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_center()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(metrics.text_secondary())
                                .text_color(cx.theme().muted_foreground)
                                .text_center()
                                .child(subtitle),
                        ),
                )
                // What to do: the three things people open this to do.
                .child(
                    div()
                        .flex()
                        .justify_center()
                        .flex_wrap()
                        .gap(metrics.space_sm())
                        .pt(metrics.space_lg())
                        .child(
                            action(
                                "info-call",
                                Icon::new(ProductIcon::Phone),
                                "Audio",
                                can_call,
                            )
                            .on_click(move |_, _, cx| {
                                audio.update(cx, |app, cx| {
                                    app.start_call(audio_jid.clone(), false, cx)
                                })
                            }),
                        )
                        .child(
                            action(
                                "info-video",
                                Icon::new(ProductIcon::Video),
                                "Video",
                                can_call,
                            )
                            .on_click(move |_, _, cx| {
                                video.update(cx, |app, cx| {
                                    app.start_call(video_jid.clone(), true, cx)
                                })
                            }),
                        )
                        .child(
                            action("info-search", Icon::new(IconName::Search), "Search", true)
                                .on_click(move |_, window, cx| {
                                    search.update(cx, |app, cx| {
                                        app.contact_info_open = false;
                                        app.toggle_conversation_search(window, cx);
                                    })
                                }),
                        ),
                )
                .children(number.clone().map(|number| {
                    let copy = number.clone();
                    div().flex().justify_center().pt(metrics.space_sm()).child(
                        Button::new("info-copy-number")
                            .ghost()
                            .small()
                            .icon(IconName::Copy)
                            .label("Copy number")
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(copy.clone()));
                            }),
                    )
                }))
                // Shared: counts, and the way into the full list.
                .child(section("Media, links and docs"))
                .child(
                    div()
                        .id("info-open-files")
                        .flex()
                        .items_center()
                        .justify_between()
                        .rounded(metrics.radius_md())
                        .px(metrics.space_md())
                        .py(metrics.space_sm())
                        .bg(cx.theme().secondary)
                        .cursor_pointer()
                        .hover(|row| row.bg(cx.theme().list_hover))
                        .on_click(move |_, _, cx| {
                            files.update(cx, |app, cx| {
                                app.contact_info_open = false;
                                app.shared_files_open = true;
                                cx.notify();
                            })
                        })
                        .child(div().text_size(metrics.text_secondary()).child(format!(
                            "{} photos · {} videos · {} docs · {} links",
                            counts.photos, counts.videos, counts.docs, counts.links
                        )))
                        .child(
                            Icon::new(IconName::ChevronRight)
                                .size(metrics.icon_small())
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(
                    div()
                        .pt(metrics.space_xxs())
                        .text_size(metrics.text_micro())
                        .text_color(cx.theme().muted_foreground)
                        .child("Counts cover the messages loaded so far."),
                )
                // About this conversation.
                .child(section("Chat"))
                .child(fact("Notifications", mute_line(chat).into()))
                .child(fact(
                    "Pinned",
                    if chat.pinned_at.is_some() {
                        "Yes"
                    } else {
                        "No"
                    }
                    .into(),
                ))
                .child(fact(
                    "Archived",
                    if chat.archived { "Yes" } else { "No" }.into(),
                ))
                // A hidden number is already said once, under the name; a
                // second "Number" row repeating it read as a glitch.
                .children(members.map(|members| {
                    div()
                        .flex()
                        .flex_col()
                        .child(section("Members"))
                        .child(members)
                }))
                .child(
                    div()
                        .pt(metrics.space_xl())
                        .flex()
                        .gap(metrics.space_sm())
                        .text_size(metrics.text_small())
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            Icon::new(ProductIcon::Lock)
                                .size(metrics.icon_small())
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            "Messages and calls are end-to-end encrypted. Only the people in \
                             this chat can read or listen to them.",
                        ),
                ),
        )
}

#[cfg(test)]
mod tests {
    use super::describe_address;

    #[test]
    fn addresses_read_as_what_they_are() {
        assert_eq!(
            describe_address("919812345678@s.whatsapp.net").0.as_deref(),
            Some("+91 98123 45678")
        );
        assert_eq!(
            describe_address("15551234567@s.whatsapp.net").0.as_deref(),
            Some("+1 555 123 4567")
        );
        assert_eq!(
            describe_address("447700900123:12@s.whatsapp.net")
                .0
                .as_deref(),
            Some("+447700900123")
        );
        let (number, note) = describe_address("221573218840689@lid");
        assert!(number.is_none(), "a privacy id is not a phone number");
        assert!(note.contains("hidden"));
    }
}

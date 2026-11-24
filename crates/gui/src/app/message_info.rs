//! "Message info": who a message this account sent was delivered to and read
//! by, and when, as WhatsApp shows it.
//!
//! On the confirmation surface, so the root's keyboard rules (Escape, the
//! focus trap) hold for it as they do for a deletion; the two never show
//! together.

use super::*;
use crate::theme::Metrics;
use gpui_component::ActiveTheme as _;
use gpui_component::FocusTrapElement as _;
use gpui_component::button::{Button, ButtonVariants as _};
use velo_core::{MessageInfo, MessageStatus, RecipientReceipt};

/// Open from one message's menu.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = message, no_json)]
pub struct ShowMessageInfo {
    pub jid: gpui::SharedString,
    pub id: gpui::SharedString,
}

pub(super) struct MessageInfoPanel {
    jid: String,
    message_id: String,
    is_group: bool,
    preview: String,
    sent_at: String,
    /// The members other than this account, where the group's roster is
    /// known: what "remaining" is counted against.
    recipients_expected: Option<usize>,
    answer: Option<Result<MessageInfo, String>>,
}

impl WhatsAppApp {
    pub(super) fn open_message_info(
        &mut self,
        jid: &str,
        message_id: &str,
        cx: &mut Context<Self>,
    ) {
        if self.delete_confirmation.is_some()
            || self.chat_confirmation.is_some()
            || self.forward_picker.is_some()
        {
            return;
        }
        let Some(chat) = self.find_chat(jid) else {
            return;
        };
        let Some(message) = chat.messages.iter().find(|m| m.id == message_id) else {
            return;
        };
        if !message.is_from_me {
            return;
        }
        let preview: String = message.preview_text().chars().take(160).collect();
        let sent_at = crate::utils::format_status_time(&message.timestamp);
        let is_group = chat.is_group;
        let recipients_expected = self
            .group_rosters
            .get(jid)
            .map(|roster| roster.members.iter().filter(|m| !m.is_self).count());
        self.message_info = Some(MessageInfoPanel {
            jid: jid.to_owned(),
            message_id: message_id.to_owned(),
            is_group,
            preview,
            sent_at,
            recipients_expected,
            answer: None,
        });
        cx.notify();
        let Some(client) = self.client.as_ref() else {
            if let Some(panel) = self.message_info.as_mut() {
                panel.answer = Some(Err("not connected to Velo's background service".into()));
            }
            return;
        };
        let answer = client.message_info(jid.to_owned(), message_id.to_owned());
        let (want_jid, want_id) = (jid.to_owned(), message_id.to_owned());
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let result = answer
                .await
                .unwrap_or_else(|_| Err("the answer never arrived".to_string()));
            let _ = entity.update(cx, |app, cx| {
                if let Some(panel) = app.message_info.as_mut()
                    && panel.jid == want_jid
                    && panel.message_id == want_id
                {
                    panel.answer = Some(result);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Close the panel, if it is up. For Escape and the close button.
    pub(super) fn close_message_info(&mut self, cx: &mut Context<Self>) -> bool {
        if self.message_info.take().is_some() {
            cx.notify();
            true
        } else {
            false
        }
    }
}

/// What a recipient is called on a line of the panel.
fn recipient_label(recipient: &RecipientReceipt) -> String {
    recipient
        .name
        .clone()
        .or_else(|| velo_core::format_phone_jid(&recipient.jid))
        .unwrap_or_else(|| crate::tr!(UnknownContact).to_string())
}

fn when(at: Option<chrono::DateTime<chrono::Utc>>) -> Option<String> {
    at.map(|at| crate::utils::format_status_time(&at))
}

/// A heading with its ticks: "✓✓ Read by".
fn section(title: &str, status: MessageStatus, metrics: Metrics, cx: &App) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap(metrics.space_sm())
        .pt(metrics.space_md())
        .child(crate::components::status_ticks(
            status,
            metrics.icon_small(),
            cx,
        ))
        .child(
            div()
                .text_size(metrics.text_secondary())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title.to_owned()),
        )
}

/// One person, or one time, under a heading.
fn line(label: String, at: Option<String>, metrics: Metrics, cx: &App) -> gpui::Div {
    let muted = cx.theme().muted_foreground;
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(metrics.space_lg())
        .pl(metrics.space_xl())
        .child(
            div()
                .min_w_0()
                .overflow_hidden()
                .text_ellipsis()
                .text_size(metrics.text_small())
                .child(label),
        )
        .children(at.map(|at| {
            div()
                .flex_shrink_0()
                .text_size(metrics.text_meta())
                .text_color(muted)
                .child(at)
        }))
}

fn render_receipts(panel: &MessageInfoPanel, metrics: Metrics, cx: &App) -> gpui::Div {
    let muted = cx.theme().muted_foreground;
    let mut body = div().flex().flex_col().gap(metrics.space_xs());
    let info = match &panel.answer {
        None => {
            return body.child(
                div()
                    .text_size(metrics.text_small())
                    .text_color(muted)
                    .child(crate::tr!(Loading)),
            );
        }
        Some(Err(detail)) => {
            return body.child(
                div()
                    .text_size(metrics.text_small())
                    .text_color(muted)
                    .child(crate::tr!(message_info_failed, detail)),
            );
        }
        Some(Ok(info)) => info,
    };
    let dash = || "\u{2014}".to_string();
    if !panel.is_group {
        // One recipient: WhatsApp draws a line per state with its time
        // rather than a list of names.
        let them = info.recipients.first();
        if let Some(at) = them.and_then(|r| r.played_at) {
            body = body
                .child(section(
                    crate::tr!(Played),
                    MessageStatus::Played,
                    metrics,
                    cx,
                ))
                .child(line(
                    crate::utils::format_status_time(&at),
                    None,
                    metrics,
                    cx,
                ));
        }
        return body
            .child(section(
                crate::tr!(ReadText),
                MessageStatus::Read,
                metrics,
                cx,
            ))
            .child(line(
                them.and_then(|r| when(r.read_time())).unwrap_or_else(dash),
                None,
                metrics,
                cx,
            ))
            .child(section(
                crate::tr!(Delivered),
                MessageStatus::Delivered,
                metrics,
                cx,
            ))
            .child(line(
                them.and_then(|r| when(r.delivered_time()))
                    .unwrap_or_else(dash),
                None,
                metrics,
                cx,
            ));
    }
    let mut read: Vec<&RecipientReceipt> =
        info.recipients.iter().filter(|r| r.has_read()).collect();
    read.sort_by_key(|r| r.read_time());
    let mut delivered: Vec<&RecipientReceipt> = info
        .recipients
        .iter()
        .filter(|r| !r.has_read() && r.delivered_at.is_some())
        .collect();
    delivered.sort_by_key(|r| r.delivered_at);
    let played: Vec<&RecipientReceipt> = info
        .recipients
        .iter()
        .filter(|r| r.played_at.is_some())
        .collect();
    if !played.is_empty() {
        body = body.child(section(
            crate::tr!(PlayedBy),
            MessageStatus::Played,
            metrics,
            cx,
        ));
        for r in &played {
            body = body.child(line(recipient_label(r), when(r.played_at), metrics, cx));
        }
    }
    body = body.child(section(
        crate::tr!(ReadBy),
        MessageStatus::Read,
        metrics,
        cx,
    ));
    if read.is_empty() {
        body = body.child(line(dash(), None, metrics, cx));
    }
    for r in &read {
        body = body.child(line(recipient_label(r), when(r.read_time()), metrics, cx));
    }
    body = body.child(section(
        crate::tr!(DeliveredTo),
        MessageStatus::Delivered,
        metrics,
        cx,
    ));
    if delivered.is_empty() {
        body = body.child(line(dash(), None, metrics, cx));
    }
    for r in &delivered {
        body = body.child(line(recipient_label(r), when(r.delivered_at), metrics, cx));
    }
    if let Some(expected) = panel.recipients_expected {
        let reached = read.len() + delivered.len();
        if expected > reached {
            body = body.child(line(
                crate::tr!(remaining_count, (expected - reached).to_string()),
                None,
                metrics,
                cx,
            ));
        }
    }
    body
}

pub(super) fn render_message_info(
    panel: &MessageInfoPanel,
    app: Entity<WhatsAppApp>,
    focus: &FocusHandle,
    cx: &App,
) -> impl IntoElement + use<> {
    let metrics = cx.product().metrics;
    let muted = cx.theme().muted_foreground;
    let close = app.clone();
    let dismiss = app;
    div()
        .id("message-info-modal")
        .debug_selector(|| "message-info-modal".into())
        .track_focus(focus)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(metrics.space_xxl())
        .bg(crate::components::parts::scrim(cx).opacity(0.92))
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            dismiss.update(cx, |app, cx| {
                app.close_message_info(cx);
            });
        })
        .child(
            div()
                .id("message-info-card")
                .w_full()
                .max_w(metrics.bubble_max_width())
                .max_h_full()
                .overflow_y_scroll()
                .p(metrics.space_xxl())
                .rounded(metrics.radius_lg())
                .bg(cx.theme().background)
                .flex()
                .flex_col()
                .gap(metrics.space_md())
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_size(metrics.text_heading())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(crate::tr!(MessageInfoTitle)),
                )
                .child(
                    div()
                        .p(metrics.space_md())
                        .rounded(metrics.radius_md())
                        .bg(cx.theme().secondary)
                        .flex()
                        .flex_col()
                        .gap(metrics.space_xs())
                        .child(
                            div()
                                .text_size(metrics.text_small())
                                .child(panel.preview.clone()),
                        )
                        .child(
                            div()
                                .text_size(metrics.text_meta())
                                .text_color(muted)
                                .child(panel.sent_at.clone()),
                        ),
                )
                .child(render_receipts(panel, metrics, cx))
                .child(
                    div().flex().justify_end().pt(metrics.space_md()).child(
                        Button::new("message-info-close")
                            .label(crate::tr!(Close))
                            .ghost()
                            .cursor_pointer()
                            .on_click(move |_, _, cx| {
                                close.update(cx, |app, cx| {
                                    app.close_message_info(cx);
                                });
                            }),
                    ),
                ),
        )
        .focus_trap("message-info-trap", focus)
}

//! What a chat row's menu does: pin, mute, archive, mark unread, clear and
//! delete.
//!
//! Each is an app-state change WhatsApp carries to the phone and every linked
//! device. The library does not echo our own change back until the next
//! app-state sync, so a confirmed change is applied to the window's copy at
//! once — a pin that waits for the round trip reads as a click that did
//! nothing. Clearing and deleting are irreversible and wait for a confirmation
//! naming the chat.

use super::*;
use gpui::SharedString;
use gpui_component::ActiveTheme as _;
use gpui_component::FocusTrapElement as _;
use gpui_component::button::{Button, ButtonVariants as _};
use oxidezap_ipc::ChatAction;

/// Pin or unpin a chat to the top of the list.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct PinChat {
    pub jid: SharedString,
    pub pin: bool,
}

/// Mute a chat's notifications for `seconds`; `None` mutes until unmuted
/// and `Some(0)` unmutes.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct MuteChat {
    pub jid: SharedString,
    pub seconds: Option<u64>,
}

/// Move a chat into, or out of, Archived.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct ArchiveChat {
    pub jid: SharedString,
    pub archive: bool,
}

/// Mark a chat unread, or read again.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct MarkChatUnread {
    pub jid: SharedString,
    pub unread: bool,
}

/// Ask to clear a chat's messages. Confirmed before anything is sent.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct ClearChat {
    pub jid: SharedString,
}

/// Ask to delete a chat. Confirmed before anything is sent.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct DeleteChat {
    pub jid: SharedString,
}

/// Eight hours and a week: the two finite mutes WhatsApp offers.
pub const MUTE_EIGHT_HOURS: u64 = 8 * 60 * 60;
pub const MUTE_ONE_WEEK: u64 = 7 * 24 * 60 * 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Irreversible {
    Clear,
    Delete,
}

/// A clear or delete waiting on the user's word.
pub(super) struct ChatConfirmation {
    pub jid: String,
    pub name: String,
    pub is_group: bool,
    pub kind: Irreversible,
}

impl WhatsAppApp {
    pub(super) fn register_chat_action_handlers(
        root: gpui::Div,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        root.on_action(cx.listener(|app, action: &PinChat, _window, cx| {
            app.run_chat_action(action.jid.to_string(), ChatAction::Pin(action.pin), cx);
        }))
        .on_action(cx.listener(|app, action: &MuteChat, _window, cx| {
            app.run_chat_action(action.jid.to_string(), ChatAction::Mute(action.seconds), cx);
        }))
        .on_action(cx.listener(|app, action: &ArchiveChat, _window, cx| {
            app.run_chat_action(
                action.jid.to_string(),
                ChatAction::Archive(action.archive),
                cx,
            );
        }))
        .on_action(cx.listener(|app, action: &MarkChatUnread, _window, cx| {
            if action.unread {
                app.run_chat_action(action.jid.to_string(), ChatAction::MarkUnread, cx);
            } else if let Some(chat) = app.find_chat(&action.jid) {
                let through = chat.messages.last().map(|m| m.id.clone());
                let jid = action.jid.to_string();
                app.mark_chat_read_from_menu(jid, through, cx);
            }
        }))
        .on_action(cx.listener(|app, action: &ClearChat, _window, cx| {
            app.begin_chat_confirmation(&action.jid, Irreversible::Clear, cx);
        }))
        .on_action(cx.listener(|app, action: &DeleteChat, _window, cx| {
            app.begin_chat_confirmation(&action.jid, Irreversible::Delete, cx);
        }))
    }

    fn mark_chat_read_from_menu(
        &mut self,
        jid: String,
        through: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(client) = self.client.as_ref() {
            client.mark_chat_read(&jid, through);
        }
        if let Some(chat) = self.find_chat_mut(&jid) {
            chat.manually_unread = false;
            chat.unread_count = 0;
        }
        self.invalidate_chat_cache();
        cx.notify();
    }

    fn begin_chat_confirmation(&mut self, jid: &str, kind: Irreversible, cx: &mut Context<Self>) {
        if self.chat_confirmation.is_some() || self.delete_confirmation.is_some() {
            return;
        }
        let Some(chat) = self.find_chat(jid) else {
            return;
        };
        let name = if chat.name.trim().is_empty() {
            jid.to_owned()
        } else {
            chat.name.clone()
        };
        self.chat_confirmation = Some(ChatConfirmation {
            jid: jid.to_owned(),
            name,
            is_group: chat.is_group,
            kind,
        });
        cx.notify();
    }

    pub(super) fn cancel_chat_confirmation(&mut self, cx: &mut Context<Self>) -> bool {
        if self.chat_confirmation.take().is_some() {
            cx.notify();
            true
        } else {
            false
        }
    }

    pub(super) fn confirm_chat_action(&mut self, cx: &mut Context<Self>) {
        let Some(confirmation) = self.chat_confirmation.take() else {
            return;
        };
        let action = match confirmation.kind {
            Irreversible::Clear => ChatAction::Clear,
            Irreversible::Delete => ChatAction::Delete,
        };
        self.run_chat_action(confirmation.jid, action, cx);
        cx.notify();
    }

    pub(super) fn run_chat_action(
        &mut self,
        jid: String,
        action: ChatAction,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = self.client.as_ref() else {
            self.notify_user(
                "Velo is not connected yet. Try again in a moment.",
                notices::Tone::Problem,
                cx,
            );
            return;
        };
        let answer = client.chat_action(jid.clone(), action.clone());
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let result = answer.await;
            let _ = entity.update(cx, |app, cx| match result {
                Ok(Ok(())) => app.apply_chat_action(&jid, &action, cx),
                Ok(Err(failure)) => app.notify_user(
                    format!("{} {failure}", chat_action_failed(&action)),
                    notices::Tone::Problem,
                    cx,
                ),
                Err(_) => app.notify_user(
                    "Velo lost its connection before WhatsApp answered.",
                    notices::Tone::Problem,
                    cx,
                ),
            });
        })
        .detach();
    }

    /// Bring the window's copy in line with a change WhatsApp accepted.
    fn apply_chat_action(&mut self, jid: &str, action: &ChatAction, cx: &mut Context<Self>) {
        let now = wacore::time::now_utc();
        match action {
            ChatAction::Delete => {
                if self.selected_chat.as_deref() == Some(jid) {
                    self.selected_chat = None;
                    self.contact_info_open = false;
                    self.shared_files_open = false;
                }
                self.departed_chats.insert(jid.to_owned());
                self.message_list_cache.borrow_mut().remove(jid);
                self.notify_user("Chat deleted", notices::Tone::Info, cx);
            }
            other => {
                if let Some(chat) = self.find_chat_mut(jid) {
                    match other {
                        ChatAction::Pin(pin) => chat.pinned_at = pin.then_some(now),
                        ChatAction::Mute(None) => {
                            chat.muted_until = Some(chrono::DateTime::<chrono::Utc>::MAX_UTC)
                        }
                        ChatAction::Mute(Some(0)) => chat.muted_until = None,
                        ChatAction::Mute(Some(seconds)) => {
                            chat.muted_until =
                                Some(now + chrono::Duration::seconds(*seconds as i64))
                        }
                        ChatAction::Archive(archive) => chat.archived = *archive,
                        ChatAction::MarkUnread => chat.manually_unread = true,
                        ChatAction::Clear => {
                            chat.messages.clear();
                            chat.last_message = None;
                        }
                        ChatAction::Delete => {}
                    }
                }
                if matches!(other, ChatAction::Clear) {
                    self.message_list_cache.borrow_mut().remove(jid);
                    self.notify_user("Chat cleared", notices::Tone::Info, cx);
                }
                if let ChatAction::Archive(archive) = other {
                    if *archive && self.selected_chat.as_deref() == Some(jid) {
                        self.selected_chat = None;
                    }
                    self.notify_user(
                        if *archive {
                            "Chat archived"
                        } else {
                            "Chat unarchived"
                        },
                        notices::Tone::Info,
                        cx,
                    );
                }
                if matches!(other, ChatAction::Pin(_))
                    && let Some(index) = self.chats.iter().position(|chat| chat.jid == jid)
                {
                    self.reposition_chat(index);
                }
            }
        }
        self.invalidate_chat_cache();
        cx.notify();
    }
}

fn chat_action_failed(action: &ChatAction) -> &'static str {
    match action {
        ChatAction::Pin(true) => "Couldn't pin this chat.",
        ChatAction::Pin(false) => "Couldn't unpin this chat.",
        ChatAction::Mute(Some(0)) => "Couldn't unmute this chat.",
        ChatAction::Mute(_) => "Couldn't mute this chat.",
        ChatAction::Archive(true) => "Couldn't archive this chat.",
        ChatAction::Archive(false) => "Couldn't unarchive this chat.",
        ChatAction::MarkUnread => "Couldn't mark this chat unread.",
        ChatAction::Clear => "Couldn't clear this chat.",
        ChatAction::Delete => "Couldn't delete this chat.",
    }
}

pub(super) fn render_chat_confirmation(
    confirmation: &ChatConfirmation,
    app: Entity<WhatsAppApp>,
    focus: &FocusHandle,
    cx: &App,
) -> impl IntoElement + use<> {
    let metrics = cx.product().metrics;
    let (title, body, confirm) = match confirmation.kind {
        Irreversible::Delete => (
            format!("Delete chat with {}?", confirmation.name),
            if confirmation.is_group {
                "The chat and its media are removed from this device, your phone and your other linked devices. You stay in the group."
            } else {
                "The chat and its media are removed from this device, your phone and your other linked devices."
            },
            "Delete chat",
        ),
        Irreversible::Clear => (
            format!("Clear this chat with {}?", confirmation.name),
            "Every message in this chat is removed on all your devices. Starred messages are kept, and the chat stays in your list.",
            "Clear chat",
        ),
    };
    let cancel = app.clone();
    div()
        .id("chat-confirm-modal")
        .debug_selector(|| "chat-confirm-modal".into())
        .track_focus(focus)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(metrics.space_xxl())
        .bg(crate::components::parts::scrim(cx).opacity(0.6))
        .on_scroll(|_, _, cx| cx.stop_propagation())
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(crate::motion::pop_in(
            div()
                .w_full()
                .max_w(metrics.bubble_max_width())
                .p(metrics.space_xxl())
                .rounded(metrics.radius_xl())
                .bg(cx.theme().popover)
                .shadow_lg()
                .flex()
                .flex_col()
                .gap(metrics.space_lg())
                .child(
                    div()
                        .text_size(metrics.text_heading())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(metrics.text_body())
                        .text_color(cx.theme().muted_foreground)
                        .child(body),
                )
                .child(
                    div()
                        .pt(metrics.space_md())
                        .flex()
                        .justify_end()
                        .gap(metrics.space_md())
                        .child(
                            Button::new("chat-confirm-cancel")
                                .label(crate::tr!(Cancel))
                                .on_click(move |_, _, cx| {
                                    cancel.update(cx, |app, cx| {
                                        app.cancel_chat_confirmation(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("chat-confirm-accept")
                                .label(confirm)
                                .danger()
                                .on_click(move |_, _, cx| {
                                    app.update(cx, |app, cx| app.confirm_chat_action(cx));
                                }),
                        ),
                ),
            "chat-confirm-card",
        ))
        .focus_trap("chat-confirm-trap", focus)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(cx: &mut gpui::TestAppContext) -> (gpui::VisualTestContext, Entity<WhatsAppApp>) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::theme::init(cx);
            init_app_bindings(cx);
        });
        let mut app_entity = None;
        let window = cx.open_window(gpui::size(gpui::px(1000.), gpui::px(800.)), |window, cx| {
            let app = cx.new(|cx| {
                let mut app = WhatsAppApp::new(cx);
                app.app_state = AppState::Connected;
                app.destination = Destination::Chats;
                let mut chat = Chat::new("peer@example.invalid".into());
                chat.name = "Test chat".into();
                app.chats.push(Arc::new(chat));
                app
            });
            app_entity = Some(app.clone());
            gpui_component::Root::new(app, window, cx)
        });
        let app = app_entity.unwrap();
        let mut cx = gpui::VisualTestContext::from_window(window.into(), cx);
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        (cx, app)
    }

    #[gpui::test]
    fn deleting_a_chat_asks_first_and_cancel_keeps_it(cx: &mut gpui::TestAppContext) {
        let (mut cx, app) = fixture(cx);
        cx.update(|window, cx| {
            window.dispatch_action(
                Box::new(DeleteChat {
                    jid: "peer@example.invalid".into(),
                }),
                cx,
            );
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("chat-confirm-modal").is_some());
        cx.update(|_window, cx| {
            app.update(cx, |app, cx| {
                let pending = app.chat_confirmation.as_ref().unwrap();
                assert_eq!(pending.kind, Irreversible::Delete);
                assert_eq!(pending.name, "Test chat");
                assert!(app.cancel_chat_confirmation(cx));
                assert!(app.find_chat("peer@example.invalid").is_some());
            });
        });
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        assert!(cx.debug_bounds("chat-confirm-modal").is_none());
    }

    #[gpui::test]
    fn an_accepted_pin_and_archive_show_at_once(cx: &mut gpui::TestAppContext) {
        let (mut cx, app) = fixture(cx);
        cx.update(|_window, cx| {
            app.update(cx, |app, cx| {
                let jid = "peer@example.invalid";
                app.apply_chat_action(jid, &ChatAction::Pin(true), cx);
                assert!(app.find_chat(jid).unwrap().pinned_at.is_some());
                app.apply_chat_action(jid, &ChatAction::Mute(Some(MUTE_EIGHT_HOURS)), cx);
                assert!(app.find_chat(jid).unwrap().muted_until.is_some());
                app.apply_chat_action(jid, &ChatAction::Mute(Some(0)), cx);
                assert!(app.find_chat(jid).unwrap().muted_until.is_none());
                app.apply_chat_action(jid, &ChatAction::Archive(true), cx);
                assert!(app.find_chat(jid).unwrap().archived);
            });
        });
    }
}

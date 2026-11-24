//! Forwarding a message: pick a chat, and the stored message goes there.
//!
//! The daemon forwards the stored proto rather than rebuilding it, so a photo
//! is relayed from the blob it already has instead of uploaded again. This
//! file is only the picker: a search box over the chat list and one row per
//! chat. A click sends; WhatsApp's own picker sends to one or many, and one
//! is the case people use.

use super::*;
use gpui::{AnyElement, SharedString};
use gpui_component::ActiveTheme as _;
use gpui_component::FocusTrapElement as _;
use gpui_component::button::ButtonVariants as _;
use gpui_component::input::{Input, InputEvent, InputState};

/// Forward one message. Opens the picker; nothing is sent until a chat is
/// chosen.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = message, no_json)]
pub struct ForwardMessage {
    pub jid: SharedString,
    pub id: SharedString,
}

/// Open the new-chat picker: search a chat, or type a number.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = chat, no_json)]
pub struct NewChat;

/// What choosing a chat in the picker does.
pub(super) enum Purpose {
    Forward {
        source_jid: String,
        message_id: String,
    },
    NewChat,
}

/// The picker while it is open.
pub(super) struct ForwardPicker {
    pub purpose: Purpose,
    pub preview: String,
    pub input: Entity<InputState>,
    /// Saved contacts, for New chat; loaded once when it opens.
    pub contacts: Vec<velo_ipc::ContactSummary>,
    pub contacts_loading: bool,
    /// Why the contacts could not be read, when they could not.
    pub contacts_error: Option<String>,
    /// The row the arrow keys are on, counted over choosable rows only.
    pub highlight: usize,
    /// What each choosable row does, in the order the last frame drew them:
    /// what Enter acts on. Written by the renderer, which is the one place
    /// that knows the order.
    pub choices: std::cell::RefCell<Vec<PickerChoice>>,
    /// The "New contact" form, while it is open in place of the list.
    pub new_contact: Option<NewContactForm>,
}

/// What one row of the picker does when chosen.
#[derive(Clone, Debug)]
pub(super) enum PickerChoice {
    Chat(String),
    Contact { jid: String, name: String },
    NewContact,
}

/// A name and a number, saved to the phone's address book through WhatsApp.
pub(super) struct NewContactForm {
    /// Which form this is. A save answers the form that sent it and no
    /// other: by the time the reply arrives the picker may have closed, or
    /// become a Forward picker, and acting on that would send a message to
    /// someone nobody chose.
    pub id: u64,
    pub name: Entity<InputState>,
    pub number: Entity<InputState>,
    pub saving: bool,
    pub error: Option<String>,
}

/// How many contacts the list draws before a search narrows it.
const CONTACTS_SHOWN: usize = 300;

/// A typed phone number as a WhatsApp address: digits only, with the country
/// code, the way WhatsApp itself asks for one. `None` for anything that is
/// not plausibly a number, so a name search never offers to message it.
pub(crate) fn phone_jid(query: &str) -> Option<String> {
    let trimmed = query.trim();
    if trimmed.is_empty()
        || !trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '+' | ' ' | '-' | '(' | ')' | '.'))
    {
        return None;
    }
    let digits: String = trimmed.chars().filter(char::is_ascii_digit).collect();
    let digits = digits.trim_start_matches('0');
    (7..=15)
        .contains(&digits.len())
        .then(|| format!("{digits}@s.whatsapp.net"))
}

/// How many chats the picker lists at once. A search narrows the rest.
const SHOWN: usize = 60;

impl WhatsAppApp {
    pub(super) fn open_forward_picker(
        &mut self,
        jid: &str,
        id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.forward_picker.is_some()
            || self.delete_confirmation.is_some()
            || self.chat_confirmation.is_some()
        {
            return;
        }
        let Some(message) = self
            .find_chat(jid)
            .and_then(|chat| chat.messages.iter().find(|m| m.id == id))
        else {
            return;
        };
        let preview: String = message.preview_text().chars().take(90).collect();
        self.open_picker(
            Purpose::Forward {
                source_jid: jid.to_owned(),
                message_id: id.to_owned(),
            },
            preview,
            "Search chats",
            window,
            cx,
        );
    }

    pub(crate) fn open_new_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.forward_picker.is_some()
            || self.delete_confirmation.is_some()
            || self.chat_confirmation.is_some()
        {
            return;
        }
        self.open_picker(
            Purpose::NewChat,
            String::new(),
            "Search a name, or type a number with its country code",
            window,
            cx,
        );
        self.load_picker_contacts(cx);
    }

    /// Fetch the saved contacts for New chat. The list shows chats until
    /// they arrive, so a slow store never leaves it empty.
    fn load_picker_contacts(&mut self, cx: &mut Context<Self>) {
        let Some(client) = self.client.as_ref() else {
            return;
        };
        if let Some(picker) = self.forward_picker.as_mut() {
            picker.contacts_loading = true;
            picker.contacts_error = None;
        }
        let answer = client.list_contacts(None, 5000);
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let result = answer.await;
            let _ = entity.update(cx, |app, cx| {
                if let Some(picker) = app.forward_picker.as_mut() {
                    picker.contacts_loading = false;
                    match result {
                        Ok(Ok(contacts)) => picker.contacts = contacts,
                        Ok(Err(error)) => picker.contacts_error = Some(error),
                        Err(_) => {
                            picker.contacts_error =
                                Some("the background service did not answer".to_string());
                        }
                    }
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// Swap the list for the New contact form, the number pre-filled when a
    /// number was already typed into the search.
    pub(super) fn open_new_contact_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.forward_picker.as_ref() else {
            return;
        };
        let typed = picker.input.read(cx).value().to_string();
        let prefill = phone_jid(&typed).map(|_| typed.trim().to_owned());
        let name = cx.new(|cx| InputState::new(window, cx).placeholder("Name"));
        let number = cx.new(|cx| {
            let state = InputState::new(window, cx)
                .placeholder("Phone number with country code, e.g. +91 98765 43210");
            match prefill {
                Some(value) => state.default_value(value),
                None => state,
            }
        });
        for input in [&name, &number] {
            cx.subscribe(input, |app, _, event: &InputEvent, cx| match event {
                InputEvent::Change => cx.notify(),
                InputEvent::PressEnter { .. } => app.save_new_contact(cx),
                _ => {}
            })
            .detach();
        }
        window.focus(&name.read(cx).focus_handle(cx), cx);
        if let Some(picker) = self.forward_picker.as_mut() {
            static NEXT_FORM: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(1);
            picker.new_contact = Some(NewContactForm {
                id: NEXT_FORM.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
                name,
                number,
                saving: false,
                error: None,
            });
        }
        cx.notify();
    }

    pub(super) fn close_new_contact_form(&mut self, cx: &mut Context<Self>) {
        if let Some(picker) = self.forward_picker.as_mut() {
            picker.new_contact = None;
        }
        cx.notify();
    }

    /// Save the form's contact, then open a chat with them.
    pub(super) fn save_new_contact(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self
            .forward_picker
            .as_mut()
            .and_then(|picker| picker.new_contact.as_mut())
        else {
            return;
        };
        if form.saving {
            return;
        }
        let name = form.name.read(cx).value().trim().to_string();
        let number = form.number.read(cx).value().to_string();
        let error = if name.is_empty() {
            Some("Enter a name.")
        } else if phone_jid(&number).is_none() {
            Some("Enter the number with its country code, like +91 98765 43210.")
        } else {
            None
        };
        if let Some(error) = error {
            form.error = Some(error.into());
            cx.notify();
            return;
        }
        let Some(jid) = phone_jid(&number) else {
            return;
        };
        let Some(client) = self.client.as_ref() else {
            form.error = Some("Velo is not connected yet. Try again in a moment.".into());
            cx.notify();
            return;
        };
        form.saving = true;
        form.error = None;
        let form_id = form.id;
        let answer = client.save_contact(jid.clone(), name.clone());
        let window = self.modal_window;
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let result = answer.await;
            let saved = matches!(result, Ok(Ok(())));
            // Whether the form that asked is still the one on screen, in a
            // New chat picker. Anything else — closed, replaced, a Forward
            // picker opened meanwhile — gets the notice and nothing more.
            let mut still_open = false;
            let _ = entity.update(cx, |app, cx| {
                still_open = app.forward_picker.as_ref().is_some_and(|picker| {
                    matches!(picker.purpose, Purpose::NewChat)
                        && picker
                            .new_contact
                            .as_ref()
                            .is_some_and(|form| form.id == form_id)
                });
                if saved {
                    app.notify_user(
                        format!("Saved {name} to your contacts"),
                        notices::Tone::Info,
                        cx,
                    );
                    return;
                }
                if still_open
                    && let Some(form) = app
                        .forward_picker
                        .as_mut()
                        .and_then(|picker| picker.new_contact.as_mut())
                {
                    form.saving = false;
                    form.error = Some(match &result {
                        Ok(Err(failure)) => format!("Couldn't save this contact. {failure}"),
                        _ => "Velo lost its connection before the contact was saved.".into(),
                    });
                }
                cx.notify();
            });
            if saved
                && still_open
                && let Some(window) = window
            {
                let _ = cx.update_window(window, |_, window, cx| {
                    let _ = entity.update(cx, |app, cx| {
                        // Straight to the chat: never through `pick_contact`,
                        // which forwards when the picker is a Forward one.
                        app.forward_picker = None;
                        app.message_person(jid, Some(name), window, cx);
                    });
                });
            }
        })
        .detach();
        cx.notify();
    }

    fn open_picker(
        &mut self,
        purpose: Purpose,
        preview: String,
        placeholder: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        cx.subscribe_in(
            &input,
            window,
            |app, _, event: &InputEvent, window, cx| match event {
                InputEvent::Change => {
                    if let Some(picker) = app.forward_picker.as_mut() {
                        picker.highlight = 0;
                    }
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => app.choose_highlighted(window, cx),
                _ => {}
            },
        )
        .detach();
        self.forward_picker = Some(ForwardPicker {
            purpose,
            preview,
            input,
            contacts: Vec::new(),
            contacts_loading: false,
            contacts_error: None,
            highlight: 0,
            choices: std::cell::RefCell::new(Vec::new()),
            new_contact: None,
        });
        cx.notify();
    }

    /// The picker's answer: forward there, or open that chat.
    pub(super) fn pick_chat(
        &mut self,
        target: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.pick_contact(target, None, window, cx);
    }

    /// As [`Self::pick_chat`], naming a chat that does not exist yet after
    /// the contact rather than their number.
    pub(super) fn pick_contact(
        &mut self,
        target: String,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.forward_picker.as_ref().map(|picker| &picker.purpose) {
            Some(Purpose::Forward { .. }) => self.forward_to(target, cx),
            Some(Purpose::NewChat) => {
                self.forward_picker = None;
                self.message_person(target, name, window, cx);
            }
            None => {}
        }
    }

    /// Open the one-to-one chat with `jid`, ready to type: the existing one
    /// when there is one, otherwise a new one. How "message this person" works
    /// from everywhere it is offered: New chat, a sender's name in a group, a
    /// group's member list.
    pub fn message_person(
        &mut self,
        jid: String,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_chat(&jid).is_none()
            && let Some(client) = self.client.as_ref()
        {
            // Not under this address, but perhaps under another of theirs:
            // a contact WhatsApp moved to a LID is stored under the LID, and
            // a number typed into New chat would open a second, empty
            // conversation beside it. The store knows; ask it first.
            let answer = client.find_chat(jid.clone());
            cx.spawn_in(window, async move |entity: WeakEntity<Self>, cx| {
                let found = answer.await.ok().and_then(Result::ok).flatten();
                let _ = entity.update_in(cx, |app, window, cx| {
                    let target = found
                        .filter(|stored| app.find_chat(stored).is_some())
                        .unwrap_or(jid);
                    app.open_person_chat(target, name, window, cx);
                });
            })
            .detach();
            return;
        }
        self.open_person_chat(jid, name, window, cx);
    }

    /// [`message_person`](Self::message_person) once the address is settled.
    fn open_person_chat(
        &mut self,
        jid: String,
        name: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.find_chat(&jid).is_none() {
            // A conversation nobody has written in yet. The store learns of
            // it with the first message; until then the window holds it so
            // there is somewhere to type.
            let mut chat = Chat::new(jid.clone());
            chat.name =
                name.unwrap_or_else(|| format!("+{}", jid.split('@').next().unwrap_or_default()));
            self.chats.insert(0, Arc::new(chat));
            self.invalidate_chat_cache();
            self.unwritten_chat = Some(jid.clone());
        }
        // Over a group's details panel the person's chat is what was asked
        // for; the panel was the group's.
        self.contact_info_open = false;
        self.select_chat(jid, ChatOpen::ToCompose, window, cx);
    }

    /// Move the picker's highlight by `step` rows, clamped to the list.
    pub(super) fn move_picker_highlight(&mut self, step: isize, cx: &mut Context<Self>) {
        let Some(picker) = self.forward_picker.as_mut() else {
            return;
        };
        let count = picker.choices.borrow().len();
        if count == 0 {
            return;
        }
        let next = picker.highlight.saturating_add_signed(step).min(count - 1);
        if next != picker.highlight {
            picker.highlight = next;
            cx.notify();
        }
    }

    /// Enter: act on the highlighted row, as a click on it would.
    pub(super) fn choose_highlighted(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(picker) = self.forward_picker.as_ref() else {
            return;
        };
        if picker.new_contact.is_some() {
            return;
        }
        let choice = picker.choices.borrow().get(picker.highlight).cloned();
        match choice {
            Some(PickerChoice::Chat(jid)) => self.pick_chat(jid, window, cx),
            Some(PickerChoice::Contact { jid, name }) => {
                self.pick_contact(jid, Some(name), window, cx);
            }
            Some(PickerChoice::NewContact) => self.open_new_contact_form(window, cx),
            None => {}
        }
    }

    pub(super) fn cancel_forward_picker(&mut self, cx: &mut Context<Self>) -> bool {
        if self.forward_picker.take().is_some() {
            cx.notify();
            true
        } else {
            false
        }
    }

    pub(super) fn forward_to(&mut self, target_jid: String, cx: &mut Context<Self>) {
        let Some(ForwardPicker {
            purpose:
                Purpose::Forward {
                    source_jid,
                    message_id,
                },
            ..
        }) = self.forward_picker.take()
        else {
            return;
        };
        cx.notify();
        let Some(client) = self.client.as_ref() else {
            self.notify_user(
                "Velo is not connected yet. Try again in a moment.",
                notices::Tone::Problem,
                cx,
            );
            return;
        };
        let name = self
            .find_chat(&target_jid)
            .map(|chat| chat.name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| target_jid.clone());
        let answer = client.forward_message(source_jid, message_id, target_jid);
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let result = answer.await;
            let _ = entity.update(cx, |app, cx| match result {
                Ok(Ok(())) => {
                    app.notify_user(format!("Forwarded to {name}"), notices::Tone::Info, cx)
                }
                Ok(Err(failure)) => app.notify_user(
                    format!("Couldn't forward that message. {failure}"),
                    notices::Tone::Problem,
                    cx,
                ),
                Err(_) => app.notify_user(
                    "Velo lost its connection before the message was forwarded.",
                    notices::Tone::Problem,
                    cx,
                ),
            });
        })
        .detach();
    }

    /// The chats the picker offers for `query`, most recent first, as the
    /// list already orders them.
    fn forward_candidates(&self, query: &str) -> Vec<Arc<Chat>> {
        let query = query.trim().to_lowercase();
        self.chats
            .iter()
            .filter(|chat| !chat.is_status)
            .filter(|chat| {
                query.is_empty()
                    || chat.name.to_lowercase().contains(&query)
                    || chat.jid.contains(&query)
            })
            .take(SHOWN)
            .cloned()
            .collect()
    }

    pub(super) fn render_forward_picker(
        &self,
        picker: &ForwardPicker,
        focus: &FocusHandle,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let metrics = cx.product().metrics;
        let theme = cx.theme();
        let query = picker.input.read(cx).value().to_string();
        let candidates = self.forward_candidates(&query);
        let media = self.media_cache();
        let entity = cx.entity().clone();
        let close = entity.clone();
        let hover = theme.list_hover;
        let avatar_size = metrics.avatar_inline();

        // What each drawn row does, kept beside the rows in the same order so
        // Enter acts on exactly the row the highlight is on. `None` is a
        // section heading, which the arrow keys skip.
        let mut row_choices: Vec<Option<PickerChoice>> = candidates
            .iter()
            .map(|chat| Some(PickerChoice::Chat(chat.jid.clone())))
            .collect();
        let rows: Vec<AnyElement> = candidates
            .iter()
            .map(|chat| {
                let target = chat.jid.clone();
                let entity = entity.clone();
                let name =
                    crate::app::chat_row::display_name(&chat.name, self.is_own_number(&chat.jid));
                div()
                    .id(SharedString::from(format!("forward-{}", chat.jid)))
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .px(metrics.space_lg())
                    .py(metrics.space_sm())
                    .rounded(metrics.radius_lg())
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, window, cx| {
                        let target = target.clone();
                        entity.update(cx, |app, cx| app.pick_chat(target, window, cx));
                    })
                    .child(
                        crate::components::Avatar::new(chat.jid.clone(), &name, avatar_size)
                            .group(chat.is_group)
                            .picture(chat.avatar_cache_key.clone(), media.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(metrics.text_body())
                            .text_color(theme.foreground)
                            .child(name),
                    )
                    .into_any_element()
            })
            .collect();
        // A typed number that is not already a chat gets its own first row,
        // the way WhatsApp's own new-chat search offers one.
        let new_number = matches!(picker.purpose, Purpose::NewChat)
            .then(|| phone_jid(&query))
            .flatten()
            .filter(|jid| self.find_chat(jid).is_none());
        let mut rows = rows;
        if let Some(jid) = new_number {
            row_choices.insert(0, Some(PickerChoice::Chat(jid.clone())));
            let entity = entity.clone();
            let label = format!("Message +{}", jid.split('@').next().unwrap_or_default());
            let target = jid.clone();
            rows.insert(
                0,
                div()
                    .id("new-chat-number")
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .px(metrics.space_lg())
                    .py(metrics.space_sm())
                    .rounded(metrics.radius_lg())
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, window, cx| {
                        let target = target.clone();
                        entity.update(cx, |app, cx| app.pick_chat(target, window, cx));
                    })
                    .child(
                        div()
                            .size(avatar_size)
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui_component::Icon::new(gpui_component::IconName::Plus)
                                    .text_color(theme.primary_foreground),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(metrics.text_body())
                            .text_color(theme.foreground)
                            .child(label),
                    )
                    .into_any_element(),
            );
        }
        let is_new_chat = matches!(picker.purpose, Purpose::NewChat);
        let section = |label: &'static str| {
            div()
                .px(metrics.space_lg())
                .pt(metrics.space_md())
                .pb(metrics.space_xs())
                .text_size(metrics.text_small())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.muted_foreground)
                .child(label)
                .into_any_element()
        };
        if is_new_chat {
            let trimmed = query.trim().to_lowercase();
            let digits: String = trimmed.chars().filter(char::is_ascii_digit).collect();
            let shown_contacts: Vec<&velo_ipc::ContactSummary> = picker
                .contacts
                .iter()
                .filter(|contact| {
                    trimmed.is_empty()
                        || contact.name.to_lowercase().contains(&trimmed)
                        || (digits.len() >= 3 && contact.jid.contains(&digits))
                })
                .take(CONTACTS_SHOWN)
                .collect();
            let contact_choices: Vec<Option<PickerChoice>> = shown_contacts
                .iter()
                .map(|contact| {
                    Some(PickerChoice::Contact {
                        jid: contact.jid.clone(),
                        name: contact.name.clone(),
                    })
                })
                .collect();
            let contact_rows: Vec<AnyElement> = shown_contacts
                .into_iter()
                .map(|contact| {
                    let entity = entity.clone();
                    let target = contact.jid.clone();
                    let name = contact.name.clone();
                    let number = format!("+{}", contact.jid.split('@').next().unwrap_or_default());
                    let picture = self
                        .find_chat(&contact.jid)
                        .and_then(|chat| chat.avatar_cache_key.clone());
                    div()
                        .id(SharedString::from(format!("contact-{}", contact.jid)))
                        .w_full()
                        .flex()
                        .items_center()
                        .gap(metrics.space_lg())
                        .px(metrics.space_lg())
                        .py(metrics.space_sm())
                        .rounded(metrics.radius_lg())
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(move |_, window, cx| {
                            let target = target.clone();
                            let name = name.clone();
                            entity.update(cx, |app, cx| {
                                app.pick_contact(target, Some(name), window, cx)
                            });
                        })
                        .child(
                            crate::components::Avatar::new(
                                contact.jid.clone(),
                                &contact.name,
                                avatar_size,
                            )
                            .picture(picture, media.clone()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(metrics.text_body())
                                        .text_color(theme.foreground)
                                        .child(contact.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(metrics.text_small())
                                        .text_color(theme.muted_foreground)
                                        .child(number),
                                ),
                        )
                        .into_any_element()
                })
                .collect();
            let have_contacts = !contact_rows.is_empty();
            let mut list: Vec<AnyElement> = Vec::new();
            let mut list_choices: Vec<Option<PickerChoice>> = vec![Some(PickerChoice::NewContact)];
            // "New contact" leads, as on the phone, whatever is typed.
            let add_entity = entity.clone();
            list.push(
                div()
                    .id("new-contact")
                    .w_full()
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .px(metrics.space_lg())
                    .py(metrics.space_sm())
                    .rounded(metrics.radius_lg())
                    .cursor_pointer()
                    .hover(move |style| style.bg(hover))
                    .on_click(move |_, window, cx| {
                        add_entity.update(cx, |app, cx| app.open_new_contact_form(window, cx));
                    })
                    .child(
                        div()
                            .size(avatar_size)
                            .flex_shrink_0()
                            .rounded_full()
                            .bg(theme.primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                gpui_component::Icon::new(gpui_component::IconName::Plus)
                                    .text_color(theme.primary_foreground),
                            ),
                    )
                    .child(
                        div()
                            .text_size(metrics.text_body())
                            .text_color(theme.foreground)
                            .child("New contact"),
                    )
                    .into_any_element(),
            );
            // While searching: the number row and matching chats (groups
            // included) first, then contacts. With no search: contacts,
            // A to Z, the way the phone lists them.
            if trimmed.is_empty() && have_contacts {
                list.push(section("Contacts on WhatsApp"));
                list_choices.push(None);
                list.extend(contact_rows);
                list_choices.extend(contact_choices);
            } else {
                if !rows.is_empty() {
                    list.push(section(if trimmed.is_empty() {
                        "Recent chats"
                    } else {
                        "Chats"
                    }));
                    list_choices.push(None);
                    list.extend(rows);
                    list_choices.append(&mut row_choices);
                }
                if have_contacts {
                    list.push(section("Contacts"));
                    list_choices.push(None);
                    list.extend(contact_rows);
                    list_choices.extend(contact_choices);
                }
            }
            // Said rather than left blank: an empty list while contacts load
            // reads as "you have none", and a failure as nothing at all.
            if picker.contacts_loading {
                list.push(section("Loading contacts…"));
                list_choices.push(None);
            } else if let Some(error) = picker.contacts_error.as_ref() {
                list.push(
                    div()
                        .px(metrics.space_lg())
                        .pt(metrics.space_md())
                        .text_size(metrics.text_small())
                        .text_color(theme.danger)
                        .child(format!("Contacts could not be loaded: {error}"))
                        .into_any_element(),
                );
                list_choices.push(None);
            }
            rows = list;
            row_choices = list_choices;
        }
        // The highlight, drawn over whichever choosable row it is on, and
        // the order recorded for Enter.
        let highlight = picker
            .highlight
            .min(row_choices.iter().flatten().count().saturating_sub(1));
        let mut choosable = 0usize;
        let rows: Vec<AnyElement> = rows
            .into_iter()
            .zip(row_choices.iter())
            .map(|(row, choice)| {
                if choice.is_none() {
                    return row;
                }
                let here = choosable == highlight;
                choosable += 1;
                if here {
                    div()
                        .rounded(metrics.radius_lg())
                        .bg(hover)
                        .child(row)
                        .into_any_element()
                } else {
                    row
                }
            })
            .collect();
        *picker.choices.borrow_mut() = row_choices.into_iter().flatten().collect();
        let empty = rows.is_empty();
        let title = match picker.purpose {
            Purpose::Forward { .. } => "Forward message to",
            Purpose::NewChat if picker.new_contact.is_some() => "New contact",
            Purpose::NewChat => "New chat",
        };
        // The New contact form, drawn in place of the search and the list.
        let form = picker.new_contact.as_ref().map(|form| {
            let back = entity.clone();
            let save = entity.clone();
            // The fields scroll and the buttons do not: in a short window
            // the card is capped at most of its height, and a footer that
            // scrolled with the fields was cut off below the fold, leaving
            // no way to save.
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("new-contact-fields")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scroll()
                        .gap(metrics.space_md())
                        .px(metrics.space_xl())
                        .child(Input::new(&form.name).w_full())
                        .child(Input::new(&form.number).w_full())
                        .child(
                            div()
                                .text_size(metrics.text_small())
                                .text_color(theme.muted_foreground)
                                .child("Saved to your phone's contacts through WhatsApp, like adding them on the phone."),
                        )
                        .children(form.error.clone().map(|error| {
                            div()
                                .text_size(metrics.text_secondary())
                                .text_color(theme.danger)
                                .child(error)
                        })),
                )
                .child(
                    div()
                        .flex_shrink_0()
                        .flex()
                        .justify_end()
                        .gap(metrics.space_md())
                        .px(metrics.space_xl())
                        .pt(metrics.space_md())
                        .pb(metrics.space_xl())
                        .child(
                            gpui_component::button::Button::new("new-contact-back")
                                .label("Back")
                                .ghost()
                                .on_click(move |_, _, cx| {
                                    back.update(cx, |app, cx| app.close_new_contact_form(cx));
                                }),
                        )
                        .child({
                            use gpui_component::Disableable as _;
                            gpui_component::button::Button::new("new-contact-save")
                                .label(if form.saving { "Saving…" } else { "Save" })
                                .primary()
                                .disabled(form.saving)
                                .on_click(move |_, _, cx| {
                                    save.update(cx, |app, cx| app.save_new_contact(cx));
                                })
                        }),
                )
                .into_any_element()
        });
        let showing_form = form.is_some();

        let keys = entity.clone();
        div()
            .id("forward-modal")
            .debug_selector(|| "forward-modal".into())
            .track_focus(focus)
            .capture_key_down(move |event, _, cx| {
                let step = match event.keystroke.key.as_str() {
                    "down" => 1,
                    "up" => -1,
                    _ => return,
                };
                cx.stop_propagation();
                keys.update(cx, |app, cx| app.move_picker_highlight(step, cx));
            })
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .p(metrics.space_xxl())
            .bg(crate::components::parts::scrim(cx).opacity(0.6))
            .on_scroll(|_, _, cx| cx.stop_propagation())
            .on_mouse_down(gpui::MouseButton::Left, move |_, _, cx| {
                cx.stop_propagation();
            })
            .child(crate::motion::pop_in(
                div()
                    .w_full()
                    .max_w(metrics.bubble_max_width())
                    .max_h(gpui::relative(0.8))
                    .rounded(metrics.radius_xl())
                    .bg(theme.popover)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .p(metrics.space_xl())
                            .flex()
                            .flex_col()
                            .gap(metrics.space_md())
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(metrics.text_heading())
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(theme.foreground)
                                            .child(title),
                                    )
                                    .child(
                                        crate::components::parts::icon_button(
                                            "forward-close",
                                            gpui_component::Icon::new(
                                                gpui_component::IconName::Close,
                                            ),
                                            crate::tr!(Cancel),
                                            metrics.icon_button(),
                                        )
                                        .on_click(
                                            move |_, _, cx| {
                                                close.update(cx, |app, cx| {
                                                    app.cancel_forward_picker(cx);
                                                });
                                            },
                                        ),
                                    ),
                            )
                            .when(!picker.preview.is_empty(), |el| {
                                el.child(
                                    div()
                                        .text_size(metrics.text_secondary())
                                        .text_color(theme.muted_foreground)
                                        .truncate()
                                        .child(picker.preview.clone()),
                                )
                            })
                            .when(!showing_form, |el| {
                                el.child(Input::new(&picker.input).w_full())
                            }),
                    )
                    .children(form)
                    .when(!showing_form, |el| el.child(
                        div()
                            .id("forward-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .px(metrics.space_md())
                            .pb(metrics.space_md())
                            .children(rows)
                            .when(empty, |el| {
                                el.child(
                                    div()
                                        .p(metrics.space_xl())
                                        .text_size(metrics.text_secondary())
                                        .text_color(theme.muted_foreground)
                                        .child("No chats match. To start one, type the number with its country code."),
                                )
                            }),
                    )),
                "forward-card",
            ))
            .focus_trap("forward-trap", focus)
    }
}

#[cfg(test)]
mod tests {
    use super::phone_jid;

    #[test]
    fn a_typed_number_becomes_an_address_and_a_name_does_not() {
        assert_eq!(
            phone_jid("+1 (555) 010-0199").as_deref(),
            Some("15550100199@s.whatsapp.net")
        );
        assert_eq!(
            phone_jid("0044 20 7946 0000").as_deref(),
            Some("442079460000@s.whatsapp.net")
        );
        assert_eq!(phone_jid("Mom"), None);
        assert_eq!(phone_jid("12345"), None);
        assert_eq!(phone_jid(""), None);
    }
}

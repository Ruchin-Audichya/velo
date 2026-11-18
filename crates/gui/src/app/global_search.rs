//! Search across every chat, from the sidebar's field.
//!
//! The sidebar's field already narrows the list to chats whose *name*
//! matches. This is the other half of what someone typing there wants: the
//! *messages* that say it, in any conversation, from the store's full-text
//! index behind the daemon — not only what this window happens to have
//! loaded. Hits are shown under the chats; picking one opens its chat and
//! scrolls to the message once it is on screen.

use gpui::{Context, Task, WeakEntity, Window};
use oxidezap_core::ChatMessage;

use super::{ChatOpen, WhatsAppApp};

/// Fewer characters than this match too much to be worth a round trip.
const MIN_QUERY_CHARS: usize = 2;
/// How many hits to ask for. A sidebar section, not a results page.
const LIMIT: u32 = 40;

/// One message that matched, already formatted for its row.
#[derive(Clone)]
pub(crate) struct GlobalHit {
    pub chat_jid: String,
    pub chat_name: String,
    pub message_id: String,
    /// Who said it, where that is worth saying: in a group, or when it was
    /// you.
    pub sender: Option<String>,
    pub snippet: String,
    pub when: String,
}

#[derive(Default)]
pub(crate) struct GlobalSearch {
    /// The query the hits answer.
    query: String,
    pub(crate) hits: Vec<GlobalHit>,
    pub(crate) loading: bool,
    /// Dropped to cancel: a newer query supersedes an older one.
    _task: Option<Task<()>>,
    /// A hit was picked whose message was not loaded yet; jump there once
    /// the chat's messages arrive.
    pub(crate) pending_jump: Option<(String, String)>,
}

impl GlobalSearch {
    pub(crate) fn is_active(&self) -> bool {
        self.loading || !self.hits.is_empty()
    }
}

impl WhatsAppApp {
    /// Re-run the search for whatever the sidebar's field now says.
    ///
    /// Called once the field's debounce has settled, so a word typed is one
    /// request rather than one per letter.
    pub(crate) fn refresh_global_search(&mut self, cx: &mut Context<Self>) {
        let query = self.search.read(cx).list_query().to_string();
        if query.chars().count() < MIN_QUERY_CHARS {
            if self.global_search.is_active() || !self.global_search.query.is_empty() {
                let pending = self.global_search.pending_jump.take();
                self.global_search = GlobalSearch {
                    pending_jump: pending,
                    ..GlobalSearch::default()
                };
                cx.notify();
            }
            return;
        }
        if query == self.global_search.query {
            return;
        }
        let Some(client) = &self.client else {
            return;
        };
        let answer = client.search_everywhere(query.clone(), LIMIT);
        self.global_search.query = query.clone();
        self.global_search.loading = true;
        self.global_search._task = Some(cx.spawn(async move |this: WeakEntity<Self>, cx| {
            let result = answer.await;
            let _ = this.update(cx, |app, cx| {
                if app.global_search.query != query {
                    return;
                }
                app.global_search.loading = false;
                app.global_search.hits = match result {
                    Ok(Ok(hits)) => hits
                        .into_iter()
                        .map(|(jid, message)| app.global_hit(jid, message))
                        .collect(),
                    _ => Vec::new(),
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn global_hit(&self, chat_jid: String, message: ChatMessage) -> GlobalHit {
        let chat = self.find_chat(&chat_jid);
        let chat_name = chat
            .map(|chat| chat.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| chat_jid.split('@').next().unwrap_or(&chat_jid).to_string());
        let is_group = chat.is_some_and(|chat| chat.is_group);
        let sender = if message.is_from_me {
            Some(crate::tr!(You).to_string())
        } else if is_group {
            message.sender_name.clone()
        } else {
            None
        };
        let mut snippet = message.preview_text();
        if snippet.chars().count() > 140 {
            snippet = snippet.chars().take(140).collect::<String>() + "…";
        }
        GlobalHit {
            chat_jid,
            chat_name,
            message_id: message.id,
            sender,
            snippet,
            when: crate::utils::format_list_time(&message.timestamp),
        }
    }

    /// Open the chat a hit is in and scroll to the message.
    pub(crate) fn open_search_hit(
        &mut self,
        chat_jid: String,
        message_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_chat(chat_jid.clone(), ChatOpen::ToCompose, window, cx);
        if !self.reveal_search_hit(&chat_jid, &message_id, cx) {
            // Not loaded yet: the chat's first page is on its way. Try again
            // when it lands — see `retry_pending_search_jump`.
            self.global_search.pending_jump = Some((chat_jid, message_id));
        }
    }

    /// Scroll to a hit if its message is loaded. Says whether it was.
    fn reveal_search_hit(
        &mut self,
        chat_jid: &str,
        message_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        let loaded = self
            .find_chat(chat_jid)
            .is_some_and(|chat| chat.messages.iter().any(|m| m.id == message_id));
        if loaded {
            self.jump_to_message(message_id, cx);
        }
        loaded
    }

    /// Finish a jump that was waiting for its chat's messages.
    pub(crate) fn retry_pending_search_jump(&mut self, cx: &mut Context<Self>) {
        let Some((jid, id)) = self.global_search.pending_jump.clone() else {
            return;
        };
        if self.selected_chat.as_deref() != Some(jid.as_str()) {
            // The reader went somewhere else; the jump no longer applies.
            self.global_search.pending_jump = None;
            return;
        }
        if self.reveal_search_hit(&jid, &id, cx) {
            self.global_search.pending_jump = None;
        }
    }
}

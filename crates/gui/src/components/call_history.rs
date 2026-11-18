use chrono::{Local, TimeZone as _};
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, Selectable as _};
use oxidezap_core::{CallHistoryEntry, format_duration};

use crate::app::WhatsAppApp;
use crate::components::ProductIcon;
use crate::theme::ActiveProductTheme as _;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Synced,
    Missed,
}

pub struct CallHistory {
    owner: WeakEntity<WhatsAppApp>,
    calls: Vec<(CallHistoryEntry, String)>,
    filter: Filter,
    loading: bool,
    error: Option<String>,
    visible: usize,
}

impl CallHistory {
    pub fn new(owner: WeakEntity<WhatsAppApp>) -> Self {
        Self {
            owner,
            calls: Vec::new(),
            filter: Filter::All,
            loading: false,
            error: None,
            visible: 100,
        }
    }

    fn load(&mut self, refresh: bool, cx: &mut Context<Self>) {
        let request = self
            .owner
            .read_with(cx, |app, _| app.call_history_request(refresh))
            .ok()
            .flatten();
        self.start(request, cx);
    }

    pub fn start(
        &mut self,
        request: Option<tokio::sync::oneshot::Receiver<Result<Vec<CallHistoryEntry>, String>>>,
        cx: &mut Context<Self>,
    ) {
        if self.loading {
            return;
        }
        let Some(request) = request else {
            self.error = Some("Call history is unavailable until your account is loaded.".into());
            cx.notify();
            return;
        };
        self.loading = true;
        self.error = None;
        let owner = self.owner.clone();
        cx.spawn(async move |view, cx| {
            let operation = async {
                request
                    .await
                    .map_err(|_| "The connection to the client ended.".to_string())?
            };
            let response = futures_lite::future::race(operation, async {
                crate::platform::clock::sleep(std::time::Duration::from_secs(60)).await;
                Err("The call history request timed out. Try again.".into())
            })
            .await;
            let _ = view.update(cx, |view, cx| {
                view.loading = false;
                match response {
                    Ok(calls) => {
                        view.calls = owner
                            .read_with(cx, |app, _| {
                                calls
                                    .into_iter()
                                    .map(|call| {
                                        let name = call
                                            .chat_jid
                                            .as_deref()
                                            .and_then(|jid| {
                                                app.chat_named(jid)
                                                    .map(|chat| chat.name.clone())
                                                    .or_else(|| {
                                                        jid.split_once('@')
                                                            .filter(|(_, server)| {
                                                                *server == "s.whatsapp.net"
                                                            })
                                                            .map(|(phone, _)| format!("+{phone}"))
                                                    })
                                            })
                                            .unwrap_or_else(|| {
                                                if call.group {
                                                    "Group call".into()
                                                } else {
                                                    "Unknown contact".into()
                                                }
                                            });
                                        (call, name)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for CallHistory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = cx.product().metrics;
        let owner = self.owner.clone();
        let calls = self
            .calls
            .iter()
            .filter(|(call, _)| match self.filter {
                Filter::All => true,
                Filter::Synced => call.synced,
                Filter::Missed => {
                    call.outcome.as_deref() == Some("missed") && call.from_me != Some(true)
                }
            })
            .collect::<Vec<_>>();
        let count = calls.len();
        let rows = calls
            .into_iter()
            .take(self.visible)
            .map(|(call, name)| {
                let direction = match call.from_me {
                    Some(true) => "Outgoing",
                    Some(false) => "Incoming",
                    None => "Unknown direction",
                };
                let media = match call.video {
                    Some(true) => "Video",
                    Some(false) => "Voice",
                    None => "Call",
                };
                let outcome = match call.outcome.as_deref() {
                    Some("connected") => "Connected",
                    Some("answered_elsewhere") => "Answered on another device",
                    // Unanswered is "missed" from the callee's side only;
                    // a call you placed that nobody picked up had no answer.
                    Some("missed") if call.from_me == Some(true) => "No answer",
                    Some("missed") => "Missed",
                    Some("declined") => "Declined",
                    Some("cancelled") => "Cancelled",
                    Some("unavailable") => "Unavailable",
                    Some("failed") => "Failed",
                    Some("abandoned") => "Abandoned",
                    _ => "Outcome unavailable",
                };
                let mut subtitle = format!("{direction} · {media} · {outcome}");
                if let Some(seconds) = call.duration_seconds {
                    subtitle.push_str(&format!(" · {}", format_duration(seconds)));
                }
                if call.synced {
                    subtitle.push_str(" · Synced");
                }
                let time = call
                    .started_ms
                    .and_then(|ms| Local.timestamp_millis_opt(ms).single())
                    .map_or_else(
                        || "Time unavailable".to_string(),
                        |time| time.format("%b %-d, %Y · %H:%M").to_string(),
                    );
                div()
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .p(metrics.space_md())
                    .rounded(metrics.radius_md())
                    .bg(cx.theme().secondary)
                    .child(
                        Icon::from(if call.video == Some(true) {
                            ProductIcon::Video
                        } else {
                            ProductIcon::Phone
                        })
                        .size(metrics.icon())
                        .text_color(if outcome == "Missed" {
                            cx.theme().danger
                        } else {
                            cx.theme().primary
                        }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().truncate().child(name.clone()))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(metrics.text_small())
                                    .text_color(cx.theme().muted_foreground)
                                    .child(subtitle),
                            ),
                    )
                    .child(
                        div()
                            .text_size(metrics.text_small())
                            .text_color(cx.theme().muted_foreground)
                            .child(time),
                    )
            })
            .collect::<Vec<_>>();
        div()
            .size_full()
            .flex()
            .flex_col()
            .gap(metrics.space_lg())
            .p(metrics.space_xl())
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(metrics.space_md())
                    .child(
                        Button::new("calls-back")
                            .ghost()
                            .label("Back to chats")
                            .on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |app, cx| {
                                    app.calls_open = false;
                                    cx.notify();
                                });
                            }),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(metrics.space_xxxl() * 2.0)
                            .text_size(metrics.text_title())
                            .child("Calls"),
                    )
                    .child(
                        Button::new("calls-refresh")
                            .ghost()
                            .label("Refresh")
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| view.load(false, cx))),
                    )
                    .child(
                        Button::new("calls-sync")
                            .outline()
                            .label("Sync from WhatsApp")
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| view.load(true, cx))),
                    ),
            )
            .child(
                div().flex().gap(metrics.space_sm()).children(
                    [
                        (Filter::All, "All calls"),
                        (Filter::Synced, "Synced"),
                        (Filter::Missed, "Missed"),
                    ]
                    .map(|(filter, label)| {
                        Button::new(label)
                            .label(label)
                            .ghost()
                            .selected(self.filter == filter)
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.filter = filter;
                                view.visible = 100;
                                cx.notify();
                            }))
                    }),
                ),
            )
            .children(
                self.error
                    .clone()
                    .map(|error| div().text_color(cx.theme().danger).child(error)),
            )
            .when(self.loading, |view| view.child("Loading call history…"))
            .when(
                count == 0 && !self.loading && self.error.is_none(),
                |view| {
                    view.child(if self.filter == Filter::All {
                        "No calls yet. Sync from WhatsApp to fetch available linked-device history."
                    } else {
                        "No calls match this filter."
                    })
                },
            )
            .child(
                div()
                    .id("call-history-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(metrics.space_sm())
                    .children(rows)
                    .when(count > self.visible, |list| {
                        list.child(
                            Button::new("calls-more")
                                .label("Show more")
                                .outline()
                                .on_click(cx.listener(|view, _, _, cx| {
                                    view.visible += 100;
                                    cx.notify();
                                })),
                        )
                    }),
            )
    }
}

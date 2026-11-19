//! The Calls screen: every call this account has a record of, newest first,
//! grouped by day, the way the phone shows them.
//!
//! A row is the person, what happened, when, and a button to call back. The
//! bookkeeping a call record carries (whether it came from a sync, the raw
//! outcome) stays out of sight: it is how the list was built, not something
//! the reader acts on.

use chrono::{Local, TimeZone as _};
use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::{ActiveTheme as _, Disableable as _, Icon, IconName, Sizable as _};
use oxidezap_core::{CallHistoryEntry, format_duration};

use crate::app::WhatsAppApp;
use crate::components::{Avatar, ProductIcon, parts};
use crate::theme::ActiveProductTheme as _;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    All,
    Missed,
}

/// One call, with what the window knows about who it was with.
struct Row {
    call: CallHistoryEntry,
    name: String,
    /// The chat to call back, when the call names one.
    jid: Option<String>,
    avatar_key: Option<String>,
}

pub struct CallHistory {
    owner: WeakEntity<WhatsAppApp>,
    calls: Vec<Row>,
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
                                calls.into_iter().map(|call| row_for(app, call)).collect()
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

fn row_for(app: &WhatsAppApp, call: CallHistoryEntry) -> Row {
    let jid = call.chat_jid.clone();
    let chat = jid.as_deref().and_then(|jid| app.chat_named(jid));
    let name = chat
        .map(|chat| chat.name.clone())
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            jid.as_deref()
                .and_then(|jid| jid.split_once('@'))
                .filter(|(_, server)| *server == "s.whatsapp.net")
                .map(|(phone, _)| format!("+{phone}"))
        })
        .unwrap_or_else(|| {
            if call.group {
                "Group call".into()
            } else {
                "Unknown contact".into()
            }
        });
    let avatar_key = chat.and_then(|chat| chat.avatar_cache_key.clone());
    Row {
        call,
        name,
        jid,
        avatar_key,
    }
}

/// A missed call the person did not answer — the one kind drawn in red.
fn missed(call: &CallHistoryEntry) -> bool {
    call.outcome.as_deref() == Some("missed") && call.from_me != Some(true)
}

/// What happened, in the words the phone uses.
fn describe(call: &CallHistoryEntry) -> String {
    let kind = if call.video == Some(true) {
        "Video"
    } else {
        "Voice"
    };
    let outcome = match call.outcome.as_deref() {
        Some("connected") => None,
        Some("answered_elsewhere") => Some("answered on another device"),
        Some("missed") if call.from_me == Some(true) => Some("no answer"),
        Some("missed") => Some("missed"),
        Some("declined") => Some("declined"),
        Some("cancelled") => Some("cancelled"),
        Some("unavailable") => Some("unavailable"),
        Some("failed") => Some("failed"),
        Some("abandoned") => Some("ended"),
        _ => None,
    };
    let mut line = match (call.from_me, outcome) {
        (_, Some(outcome)) => format!("{kind} · {outcome}"),
        (Some(true), None) => format!("Outgoing {}", kind.to_lowercase()),
        (_, None) => format!("Incoming {}", kind.to_lowercase()),
    };
    if let Some(seconds) = call.duration_seconds.filter(|s| *s > 0) {
        line.push_str(&format!(" · {}", format_duration(seconds)));
    }
    line
}

/// The day heading a call sits under: Today, Yesterday, the weekday within
/// a week, else the date.
fn day_label(day: chrono::NaiveDate, today: chrono::NaiveDate) -> String {
    match (today - day).num_days() {
        0 => "Today".into(),
        1 => "Yesterday".into(),
        2..=6 => day.format("%A").to_string(),
        _ => day.format("%-d %B %Y").to_string(),
    }
}

impl Render for CallHistory {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = cx.product().metrics;
        let today = wacore::time::now_utc().with_timezone(&Local).date_naive();
        let rows: Vec<&Row> = self
            .calls
            .iter()
            .filter(|row| match self.filter {
                Filter::All => true,
                Filter::Missed => missed(&row.call),
            })
            .collect();
        let count = rows.len();

        let mut list: Vec<AnyElement> = Vec::new();
        let mut last_day = None;
        for (index, row) in rows.into_iter().take(self.visible).enumerate() {
            let local = row
                .call
                .started_ms
                .and_then(|ms| Local.timestamp_millis_opt(ms).single());
            let day = local.map(|time| time.date_naive());
            if day != last_day {
                last_day = day;
                list.push(
                    div()
                        .px(metrics.space_lg())
                        .pt(metrics.space_lg())
                        .pb(metrics.space_xs())
                        .text_size(metrics.text_small())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(day.map_or_else(|| "Earlier".into(), |d| day_label(d, today)))
                        .into_any_element(),
                );
            }
            let time = local.map_or_else(String::new, |t| t.format("%H:%M").to_string());
            let is_missed = missed(&row.call);
            let outgoing = row.call.from_me == Some(true);
            let video = row.call.video == Some(true);
            let hover = cx.theme().list_hover;
            let call_back = row.jid.clone().filter(|_| !row.call.group).map(|jid| {
                let owner = self.owner.clone();
                Button::new(("call-back", index))
                    .ghost()
                    .small()
                    .icon(Icon::from(if video {
                        ProductIcon::Video
                    } else {
                        ProductIcon::Phone
                    }))
                    .tooltip(if video { "Video call" } else { "Voice call" })
                    .on_click(move |_, _, cx| {
                        let jid = jid.clone();
                        let _ = owner.update(cx, |app, cx| app.start_call(jid, video, cx));
                    })
            });
            let identity = row.jid.clone().unwrap_or_else(|| row.call.id.clone());
            list.push(
                div()
                    .id(("call-row", index))
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .h(metrics.chat_row_height())
                    .px(metrics.space_lg())
                    .rounded(metrics.radius_lg())
                    .hover(move |style| style.bg(hover))
                    .child(
                        Avatar::new(identity, &row.name, metrics.avatar_row())
                            .group(row.call.group)
                            .picture(row.avatar_key.clone(), None),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(metrics.space_xxs())
                            .child(
                                div()
                                    .truncate()
                                    .text_size(metrics.text_body())
                                    .text_color(if is_missed {
                                        cx.theme().danger
                                    } else {
                                        cx.theme().foreground
                                    })
                                    .child(row.name.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(metrics.space_xs())
                                    .min_w_0()
                                    .text_size(metrics.text_secondary())
                                    .text_color(parts::subtle(cx))
                                    .child(
                                        Icon::new(if outgoing {
                                            IconName::ArrowUp
                                        } else {
                                            IconName::ArrowDown
                                        })
                                        .size(metrics.icon_small())
                                        .text_color(
                                            if is_missed {
                                                cx.theme().danger
                                            } else {
                                                cx.theme().primary
                                            },
                                        ),
                                    )
                                    .child(div().truncate().child(describe(&row.call))),
                            ),
                    )
                    .child(
                        div()
                            .text_size(metrics.text_meta())
                            .text_color(parts::subtle(cx))
                            .child(time),
                    )
                    .children(call_back)
                    .into_any_element(),
            );
        }

        let filter_chip = |filter: Filter, label: &'static str, cx: &mut Context<Self>| {
            let selected = self.filter == filter;
            Button::new(label)
                .label(label)
                .small()
                .rounded(metrics.radius_xl())
                .when(selected, |b| b.primary())
                .when(!selected, |b| b.ghost())
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.filter = filter;
                    view.visible = 100;
                    cx.notify();
                }))
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .h(metrics.sidebar_header_height())
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(metrics.space_xl())
                    .child(
                        div()
                            .text_size(metrics.text_title())
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Calls"),
                    )
                    .child(
                        Button::new("calls-sync")
                            .ghost()
                            .small()
                            .icon(IconName::Redo)
                            .tooltip("Fetch call history from your phone")
                            .disabled(self.loading)
                            .on_click(cx.listener(|view, _, _, cx| view.load(true, cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap(metrics.space_sm())
                    .px(metrics.space_xl())
                    .pb(metrics.space_md())
                    .child(filter_chip(Filter::All, "All", cx))
                    .child(filter_chip(Filter::Missed, "Missed", cx)),
            )
            .children(self.error.clone().map(|error| {
                div()
                    .px(metrics.space_xl())
                    .text_color(cx.theme().danger)
                    .child(error)
            }))
            .when(self.loading && count == 0, |view| {
                view.child(
                    div()
                        .px(metrics.space_xl())
                        .text_color(parts::subtle(cx))
                        .child("Loading your calls…"),
                )
            })
            .when(
                count == 0 && !self.loading && self.error.is_none(),
                |view| {
                    view.child(
                        div()
                            .px(metrics.space_xl())
                            .pt(metrics.space_lg())
                            .text_color(parts::subtle(cx))
                            .child(if self.filter == Filter::All {
                                "No calls yet. Calls you make and get show up here."
                            } else {
                                "No missed calls."
                            }),
                    )
                },
            )
            .child(
                div()
                    .id("call-history-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .px(metrics.space_md())
                    .pb(metrics.space_lg())
                    .flex()
                    .flex_col()
                    .children(list)
                    .when(count > self.visible, |list| {
                        list.child(
                            div().p(metrics.space_lg()).child(
                                Button::new("calls-more")
                                    .label("Show more")
                                    .ghost()
                                    .on_click(cx.listener(|view, _, _, cx| {
                                        view.visible += 100;
                                        cx.notify();
                                    })),
                            ),
                        )
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    // Named, not `super::*`: the glob brings gpui's own `test` macro along,
    // which shadows the standard one.
    use super::{CallHistoryEntry, day_label, describe, missed};

    fn call(from_me: bool, outcome: &str, seconds: Option<u32>) -> CallHistoryEntry {
        CallHistoryEntry {
            id: "c".into(),
            chat_jid: None,
            caller_jid: None,
            started_ms: None,
            from_me: Some(from_me),
            video: Some(false),
            group: false,
            duration_seconds: seconds,
            outcome: Some(outcome.into()),
            synced: true,
        }
    }

    #[test]
    fn a_call_reads_the_way_the_phone_says_it() {
        assert_eq!(
            describe(&call(true, "connected", Some(133))),
            "Outgoing voice · 2:13"
        );
        assert_eq!(describe(&call(false, "missed", None)), "Voice · missed");
        assert_eq!(
            describe(&call(true, "missed", Some(0))),
            "Voice · no answer"
        );
        assert!(missed(&call(false, "missed", None)));
        assert!(!missed(&call(true, "missed", None)));
    }

    #[test]
    fn days_are_named_like_the_phone_names_them() {
        let today = chrono::NaiveDate::from_ymd_opt(2026, 11, 19).unwrap();
        assert_eq!(day_label(today, today), "Today");
        assert_eq!(day_label(today.pred_opt().unwrap(), today), "Yesterday");
        let earlier = chrono::NaiveDate::from_ymd_opt(2026, 10, 2).unwrap();
        assert_eq!(day_label(earlier, today), "2 October 2026");
    }
}

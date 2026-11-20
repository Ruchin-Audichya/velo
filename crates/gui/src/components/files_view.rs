//! A searchable, paginated view of the client's categorized downloads.

use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _};

use crate::app::WhatsAppApp;
use crate::platform::files::{self, Category, Entry};
use crate::theme::ActiveProductTheme as _;
use chrono::TimeZone as _;

pub struct FilesView {
    owner: WeakEntity<WhatsAppApp>,
    search: Entity<InputState>,
    /// Documents shared in any chat whose name matches the search, from the
    /// daemon's trigram index: found whether or not they were downloaded.
    shared: Vec<SharedFile>,
    /// Bumped by every keystroke, so a slower, older answer never replaces
    /// a newer one.
    search_generation: u64,
    entries: Vec<Entry>,
    category: Category,
    sort: usize,
    visible: usize,
    loading: bool,
    error: Option<String>,
}

/// One document from the index, with the chat it was shared in.
struct SharedFile {
    found: velo_ipc::FileMatch,
    chat_name: String,
}

impl FilesView {
    pub fn new(
        owner: WeakEntity<WhatsAppApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Search files in every chat — any part of the name")
        });
        cx.subscribe(&search, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.visible = 100;
                view.search_shared(cx);
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            owner,
            search,
            shared: Vec::new(),
            search_generation: 0,
            entries: Vec::new(),
            category: Category::All,
            sort: 0,
            visible: 100,
            loading: false,
            error: None,
        };
        view.refresh(cx);
        view.search_shared(cx);
        view
    }

    /// Query the index for what is typed, a moment after typing stops.
    fn search_shared(&mut self, cx: &mut Context<Self>) {
        self.search_generation += 1;
        let generation = self.search_generation;
        let query = self.search.read(cx).value().trim().to_string();
        let owner = self.owner.clone();
        cx.spawn(async move |view, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(120))
                .await;
            let current = view
                .read_with(cx, |view, _| view.search_generation == generation)
                .unwrap_or(false);
            if !current {
                return;
            }
            let Some(request) = owner
                .read_with(cx, |app, _| app.search_files_request(query, 100))
                .ok()
                .flatten()
            else {
                return;
            };
            let Ok(Ok(found)) = request.await else {
                return;
            };
            let shared: Vec<SharedFile> = owner
                .read_with(cx, |app, _| {
                    found
                        .into_iter()
                        .map(|found| SharedFile {
                            chat_name: app
                                .chat_named(&found.chat_jid)
                                .map(|chat| chat.name.clone())
                                .filter(|name| !name.trim().is_empty())
                                .unwrap_or_else(|| "A chat".into()),
                            found,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let _ = view.update(cx, |view, cx| {
                if view.search_generation == generation {
                    view.shared = shared;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        self.loading = true;
        self.error = None;
        let task = cx.background_executor().spawn(async { files::scan() });
        cx.spawn(async move |entity, cx| {
            let result = task.await;
            let _ = entity.update(cx, |view, cx| {
                view.loading = false;
                match result {
                    Ok(entries) => view.entries = entries,
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

impl Render for FilesView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let metrics = cx.product().metrics;
        let query = self.search.read(cx).value().to_lowercase();
        let mut entries: Vec<_> = self
            .entries
            .iter()
            .filter(|entry| {
                (self.category == Category::All || self.category == entry.category)
                    && entry.name.to_lowercase().contains(&query)
            })
            .collect();
        match self.sort {
            1 => entries.sort_by_cached_key(|entry| entry.name.to_lowercase()),
            2 => entries.sort_by(|a, b| b.size.cmp(&a.size).then(a.name.cmp(&b.name))),
            _ => entries.sort_by(|a, b| b.modified.cmp(&a.modified).then(a.name.cmp(&b.name))),
        }
        let count = entries.len();
        let rows = entries
            .into_iter()
            .take(self.visible)
            .map(|entry| {
                let open = entry.path.clone();
                let reveal = entry.path.clone();
                let key = entry.path.to_string_lossy().into_owned();
                let name = entry.name.clone();
                let detail = format!(
                    "{} · {}",
                    entry.category.label(),
                    files::size_label(entry.size)
                );
                // The same coloured type tile the chat's own Files panel and
                // document bubbles use, so a spreadsheet looks like one here
                // too, instead of every file being the same grey page.
                let kind = match entry.category {
                    Category::Images => crate::components::file_badge::FileKind::Image,
                    Category::Videos => crate::components::file_badge::FileKind::Video,
                    Category::Audio => crate::components::file_badge::FileKind::Audio,
                    Category::Stickers => crate::components::file_badge::FileKind::Sticker,
                    _ => crate::components::file_badge::FileKind::Document,
                };
                let label = crate::components::file_badge::extension_label(&entry.name, "");
                let hover = cx.theme().list_hover;
                div()
                    .id(SharedString::from(key.clone()))
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .h(metrics.chat_row_height())
                    .px(metrics.space_lg())
                    .rounded(metrics.radius_lg())
                    .hover(move |style| style.bg(hover))
                    .child(crate::components::file_badge::file_badge(
                        kind,
                        label.as_deref(),
                        metrics.avatar_row(),
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().truncate().child(name))
                            .child(
                                div()
                                    .text_size(metrics.text_small())
                                    .text_color(cx.theme().muted_foreground)
                                    .child(detail),
                            ),
                    )
                    .child(
                        Button::new(SharedString::from(format!("open-{key}")))
                            .icon(IconName::ExternalLink)
                            .tooltip("Open")
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                cx.open_with_system(&open);
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("reveal-{key}")))
                            .icon(IconName::FolderOpen)
                            .tooltip("Show in folder")
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                cx.reveal_path(&reveal);
                            }),
                    )
            })
            .collect::<Vec<_>>();

        // Documents from the index: everything shared in any chat, which the
        // downloads folder alone cannot list. Shown for All and Documents,
        // the two filters a document belongs to.
        let show_shared = matches!(self.category, Category::All | Category::Documents);
        let shared_rows: Vec<AnyElement> = if show_shared {
            self.shared
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let found = &item.found;
                    let label = crate::components::file_badge::extension_label(
                        &found.file_name,
                        found.mime.as_deref().unwrap_or(""),
                    );
                    let when = chrono::Local
                        .timestamp_millis_opt(found.timestamp_ms)
                        .single()
                        .map(|time| time.format("%-d %b %Y").to_string())
                        .unwrap_or_default();
                    let mut detail = format!("{} · {when}", item.chat_name);
                    if let Some(size) = found.size {
                        detail.push_str(&format!(" · {}", files::size_label(size)));
                    }
                    let owner = self.owner.clone();
                    let chat = found.chat_jid.clone();
                    let message = found.msg_id.clone();
                    let hover = cx.theme().list_hover;
                    div()
                        .id(("shared-file", index))
                        .flex()
                        .items_center()
                        .gap(metrics.space_lg())
                        .h(metrics.chat_row_height())
                        .px(metrics.space_lg())
                        .rounded(metrics.radius_lg())
                        .cursor_pointer()
                        .hover(move |style| style.bg(hover))
                        .on_click(move |_, window, cx| {
                            let chat = chat.clone();
                            let message = message.clone();
                            let _ = owner.update(cx, |app, cx| {
                                app.files_open = false;
                                app.open_search_hit(chat, message, window, cx);
                            });
                        })
                        .child(crate::components::file_badge::file_badge(
                            crate::components::file_badge::FileKind::Document,
                            label.as_deref(),
                            metrics.avatar_row(),
                            cx,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().child(found.file_name.clone()))
                                .child(
                                    div()
                                        .truncate()
                                        .text_size(metrics.text_small())
                                        .text_color(cx.theme().muted_foreground)
                                        .child(detail),
                                ),
                        )
                        .into_any_element()
                })
                .collect()
        } else {
            Vec::new()
        };
        let searching = !self.search.read(cx).value().trim().is_empty();
        let shared_heading = if searching {
            "In your chats"
        } else {
            "Recently shared in your chats"
        };
        let section = |label: String| {
            div()
                .pt(metrics.space_md())
                .pb(metrics.space_xs())
                .px(metrics.space_lg())
                .text_size(metrics.text_small())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(label)
                .into_any_element()
        };
        let mut list: Vec<AnyElement> = Vec::new();
        if !rows.is_empty() {
            list.push(section(format!("Downloaded · {count}")));
            list.extend(rows.into_iter().map(IntoElement::into_any_element));
        }
        if !shared_rows.is_empty() {
            list.push(section(format!("{shared_heading} · {}", shared_rows.len())));
            list.extend(shared_rows);
        }
        let nothing = list.is_empty();

        div().size_full().flex().flex_col().gap(metrics.space_md()).px(metrics.space_xl()).pb(metrics.space_lg())
            .bg(cx.theme().background).text_color(cx.theme().foreground)
            // The rail is the way back, as on every other screen.
            .child(div().h(metrics.sidebar_header_height()).flex_shrink_0().flex().items_center().justify_between()
                .child(div().text_size(metrics.text_title()).font_weight(FontWeight::SEMIBOLD).child("Files"))
                .child(div().flex().gap(metrics.space_xs())
                    .child(Button::new("files-folder").icon(IconName::FolderOpen).tooltip("Open the downloads folder").ghost().small()
                        .on_click(|_, _, cx| { if let Ok(path) = files::root() { cx.open_with_system(&path); } }))
                    .child(Button::new("files-refresh").icon(IconName::Redo).tooltip("Refresh").ghost().small().disabled(self.loading)
                        .on_click(cx.listener(|view, _, _, cx| view.refresh(cx))))))
            .child(Input::new(&self.search))
            .child(div().flex().flex_wrap().gap(metrics.space_sm())
                .children(Category::ALL.into_iter().map(|category| {
                    let selected = self.category == category;
                    Button::new(category.label()).label(category.label()).small().rounded(metrics.radius_xl())
                        .when(selected, |b| b.primary()).when(!selected, |b| b.ghost())
                        .on_click(cx.listener(move |view, _, _, cx| { view.category = category; view.visible = 100; cx.notify(); }))
                })))
            .child(div().flex().items_center().justify_between()
                .child(format!("{count} files"))
                .child(Button::new("files-sort").label(["Newest first", "Name A–Z", "Largest first"][self.sort]).ghost()
                    .on_click(cx.listener(|view, _, _, cx| { view.sort = (view.sort + 1) % 3; cx.notify(); }))))
            .children(self.error.clone().map(|error| div().text_color(cx.theme().danger).child(error)))
            .when(self.loading, |view| view.child("Loading downloads…"))
            .when(nothing && !self.loading && self.error.is_none(), |view| view.child(
                div().px(metrics.space_lg()).text_color(cx.theme().muted_foreground).child(
                    "No files match. Every document shared in your chats can be found here by any part of its name.")))
            .child(div().id("files-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col()
                .children(list)
                .when(count > self.visible, |list| list.child(Button::new("files-more").label("Show more").outline()
                    .on_click(cx.listener(|view, _, _, cx| { view.visible += 100; cx.notify(); })))))
    }
}

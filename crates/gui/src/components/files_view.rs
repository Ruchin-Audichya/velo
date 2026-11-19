//! A searchable, paginated view of the client's categorized downloads.

use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _};

use crate::app::WhatsAppApp;
use crate::platform::files::{self, Category, Entry};
use crate::theme::ActiveProductTheme as _;

pub struct FilesView {
    search: Entity<InputState>,
    entries: Vec<Entry>,
    category: Category,
    sort: usize,
    visible: usize,
    loading: bool,
    error: Option<String>,
}

impl FilesView {
    pub fn new(
        // Kept in the signature for the caller; the rail, not a back
        // button here, is now the way out, so nothing reads it.
        _owner: WeakEntity<WhatsAppApp>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search downloaded files…"));
        cx.subscribe(&search, |view, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                view.visible = 100;
                cx.notify();
            }
        })
        .detach();
        let mut view = Self {
            search,
            entries: Vec::new(),
            category: Category::All,
            sort: 0,
            visible: 100,
            loading: false,
            error: None,
        };
        view.refresh(cx);
        view
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
            .when(count == 0 && !self.loading && self.error.is_none(), |view| view.child(
                "Saved attachments appear here automatically. Download a file from a conversation to get started."))
            .child(div().id("files-list").flex_1().min_h_0().overflow_y_scroll().flex().flex_col()
                .children(rows)
                .when(count > self.visible, |list| list.child(Button::new("files-more").label("Show more").outline()
                    .on_click(cx.listener(|view, _, _, cx| { view.visible += 100; cx.notify(); })))))
    }
}

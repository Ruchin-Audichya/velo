//! A searchable, paginated view of the client's categorized downloads.

use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Selectable as _, Sizable as _};

use crate::app::WhatsAppApp;
use crate::platform::files::{self, Category, Entry};
use crate::theme::ActiveProductTheme as _;

pub struct FilesView {
    owner: WeakEntity<WhatsAppApp>,
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
        owner: WeakEntity<WhatsAppApp>,
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
            owner,
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
        let owner = self.owner.clone();
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
                div()
                    .id(SharedString::from(key.clone()))
                    .flex()
                    .items_center()
                    .gap(metrics.space_lg())
                    .p(metrics.space_md())
                    .rounded(metrics.radius_md())
                    .bg(cx.theme().secondary)
                    .child(gpui_component::Icon::new(IconName::File).size(metrics.icon()))
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
                            .label("Open")
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                cx.open_with_system(&open);
                            }),
                    )
                    .child(
                        Button::new(SharedString::from(format!("reveal-{key}")))
                            .label("Show in folder")
                            .ghost()
                            .small()
                            .on_click(move |_, _, cx| {
                                cx.reveal_path(&reveal);
                            }),
                    )
            })
            .collect::<Vec<_>>();

        div().size_full().flex().flex_col().gap(metrics.space_lg()).p(metrics.space_xl())
            .bg(cx.theme().background).text_color(cx.theme().foreground)
            .child(div().flex().flex_wrap().items_center().gap(metrics.space_md())
                .child(Button::new("files-back").label("Back to chats").ghost().on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |app, cx| { app.files_open = false; cx.notify(); });
                }))
                .child(div().flex_1().min_w(metrics.space_xxxl() * 3.0).text_size(metrics.text_title()).child("Files & downloads"))
                .child(Button::new("files-folder").label("Downloads folder").outline().on_click(|_, _, cx| {
                    if let Ok(path) = files::root() { cx.open_with_system(&path); }
                }))
                .child(Button::new("files-refresh").label("Refresh").outline().disabled(self.loading)
                    .on_click(cx.listener(|view, _, _, cx| view.refresh(cx)))))
            .child(Input::new(&self.search))
            .child(div().flex().flex_wrap().gap(metrics.space_sm())
                .children(Category::ALL.into_iter().map(|category| {
                    Button::new(category.label()).label(category.label()).ghost().selected(self.category == category)
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
                .gap(metrics.space_sm()).children(rows)
                .when(count > self.visible, |list| list.child(Button::new("files-more").label("Show more").outline()
                    .on_click(cx.listener(|view, _, _, cx| { view.visible += 100; cx.notify(); })))))
    }
}

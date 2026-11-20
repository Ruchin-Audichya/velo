//! The conversation's files, down the right of the chat.
//!
//! Built for finding one file fast: a search over names and captions, a tab
//! per kind with how many there are, newest-or-largest ordering, a heading
//! per month, and a badge that says what a file is before its name does.
//! Photos open in the viewer (a preview, nothing saved); everything else
//! downloads or reveals in one click.

use gpui::{prelude::*, *};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme as _, Disableable as _, IconName, Sizable as _};
use velo_core::{Chat, ChatMessage, MediaContent, MediaType};

use crate::app::WhatsAppApp;
use crate::components::file_badge::{FileKind, extension_label, file_badge};
use crate::theme::Metrics;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Filter {
    #[default]
    All,
    Images,
    Videos,
    Documents,
    Audio,
    Stickers,
}

impl Filter {
    const ALL: [(Filter, &'static str); 6] = [
        (Filter::All, "All"),
        (Filter::Documents, "Docs"),
        (Filter::Images, "Photos"),
        (Filter::Videos, "Videos"),
        (Filter::Audio, "Audio"),
        (Filter::Stickers, "Stickers"),
    ];

    fn accepts(self, kind: &MediaType) -> bool {
        match self {
            Self::All => true,
            Self::Images => *kind == MediaType::Image,
            Self::Videos => *kind == MediaType::Video,
            Self::Documents => *kind == MediaType::Document,
            Self::Audio => *kind == MediaType::Audio,
            Self::Stickers => *kind == MediaType::Sticker,
        }
    }
}

/// How the list is ordered.
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Newest,
    Largest,
}

/// One file in the conversation, with what the panel shows about it.
struct Entry<'a> {
    message: &'a ChatMessage,
    media: &'a MediaContent,
    kind: MediaType,
    name: String,
    bytes: u64,
}

fn kind_of(media: &MediaContent) -> MediaType {
    if media.is_audio_attachment() {
        MediaType::Audio
    } else {
        media.media_type.clone()
    }
}

fn display_name(media: &MediaContent) -> String {
    media.file_name.clone().unwrap_or_else(|| {
        match media.media_type {
            MediaType::Image => "Photo",
            MediaType::Video => "Video",
            MediaType::Audio => "Voice message",
            MediaType::Document => "Document",
            MediaType::Sticker => "Sticker",
        }
        .into()
    })
}

fn size_of(media: &MediaContent) -> u64 {
    media
        .downloadable
        .as_ref()
        .map(|d| d.file_length)
        .filter(|len| *len > 0)
        .unwrap_or_else(|| {
            if media.data_is_preview {
                0
            } else {
                media.data.len() as u64
            }
        })
}

/// Every file in the loaded messages that can be opened or fetched.
fn entries(chat: &Chat) -> Vec<Entry<'_>> {
    chat.messages
        .iter()
        .filter_map(|message| {
            let media = message.media.as_ref()?;
            let fetchable =
                media.downloadable.is_some() || !(media.data_is_preview || media.data.is_empty());
            (!message.revoked && fetchable).then(|| Entry {
                message,
                media,
                kind: kind_of(media),
                name: display_name(media),
                bytes: size_of(media),
            })
        })
        .collect()
}

/// Whether `entry` matches a lower-cased search over its name, its caption,
/// and its type label ("pdf" finds every PDF).
fn matches(entry: &Entry<'_>, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    entry.name.to_lowercase().contains(query)
        || entry
            .media
            .caption
            .as_deref()
            .is_some_and(|caption| caption.to_lowercase().contains(query))
        || badge_label(entry).to_lowercase().contains(query)
}

/// The short type label on a file's badge: its extension where it has one.
fn badge_label(entry: &Entry<'_>) -> String {
    let from_name = entry
        .name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && ext.len() <= 5);
    // Office and archive types have long MIME names; say them the way a
    // file manager would.
    let mime = entry.media.mime_type.as_str();
    let known = [
        ("spreadsheetml", "xlsx"),
        ("ms-excel", "xls"),
        ("wordprocessingml", "docx"),
        ("msword", "doc"),
        ("presentationml", "pptx"),
        ("ms-powerpoint", "ppt"),
        ("pdf", "pdf"),
        ("zip", "zip"),
    ]
    .into_iter()
    .find(|(needle, _)| mime.contains(needle))
    .map(|(_, short)| short);
    let from_mime = known.or_else(|| {
        mime.rsplit_once('/')
            .map(|(_, sub)| sub.split(['.', '-', '+', ';']).next_back().unwrap_or(sub))
    });
    match entry.kind {
        MediaType::Image => from_name.or(from_mime).unwrap_or("img"),
        MediaType::Video => from_name.unwrap_or("mp4"),
        MediaType::Audio => from_name.unwrap_or("opus"),
        MediaType::Sticker => "webp",
        MediaType::Document => from_name.or(from_mime).unwrap_or("file"),
    }
    .to_uppercase()
    .chars()
    .take(4)
    .collect()
}

fn format_size(bytes: u64) -> Option<String> {
    if bytes == 0 {
        return None;
    }
    let units = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < units.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    Some(if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units[unit])
    })
}

pub fn render(
    app: &WhatsAppApp,
    chat: &Chat,
    filter: Filter,
    can_load_older: bool,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> impl IntoElement + use<> {
    let close = entity.clone();
    let more = entity.clone();
    let jid = chat.jid.clone();
    let sort = app.shared_files_sort;
    let query = app
        .shared_files_search
        .as_ref()
        .map(|input| input.read(cx).value().trim().to_lowercase())
        .unwrap_or_default();

    let all = entries(chat);
    let searched: Vec<&Entry<'_>> = all.iter().filter(|e| matches(e, &query)).collect();

    // A count per tab, over what the search leaves.
    // A tab bar, as in WhatsApp's media panel: the active tab in the accent
    // with a bar under it, the rest quiet, each with its count. A ghost
    // button's own selected state was a shade off its resting one, which on
    // a dark theme read as no selection at all.
    // Only the kinds this chat has, plus whichever is chosen: six tabs in a
    // narrow panel wrapped onto a second row, and "Stickers" with nothing
    // under it is a tab that only says no.
    let tabs = Filter::ALL
        .into_iter()
        .filter_map(|(choice, label)| {
            let count = searched.iter().filter(|e| choice.accepts(&e.kind)).count();
            let active = filter == choice;
            if count == 0 && !active && choice != Filter::All {
                return None;
            }
            let entity = entity.clone();
            let tab = div()
                .id(SharedString::from(format!("files-tab-{label}")))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(metrics.space_xs())
                .px(metrics.space_md())
                .py(metrics.space_sm())
                .border_b_2()
                .border_color(if active {
                    cx.theme().primary
                } else {
                    gpui::transparent_black()
                })
                .text_size(metrics.text_small())
                .font_weight(if active {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::NORMAL
                })
                .text_color(if active {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .cursor_pointer()
                .hover(|style| style.text_color(cx.theme().foreground))
                .child(label)
                .when(count > 0, |tab| {
                    tab.child(
                        div()
                            .text_size(metrics.text_micro())
                            .font_features(crate::components::parts::tabular())
                            .opacity(0.8)
                            .child(count.to_string()),
                    )
                })
                .on_click(move |_, _, cx| {
                    entity.update(cx, |app, cx| {
                        app.shared_files_filter = choice;
                        cx.notify();
                    })
                });
            Some(tab)
        })
        .collect::<Vec<_>>();

    let mut shown: Vec<&Entry<'_>> = searched
        .into_iter()
        .filter(|e| filter.accepts(&e.kind))
        .collect();
    match sort {
        Sort::Newest => shown.sort_by_key(|e| std::cmp::Reverse(e.message.timestamp)),
        Sort::Largest => shown.sort_by_key(|e| std::cmp::Reverse(e.bytes)),
    }
    shown.truncate(300);
    let empty = shown.is_empty();

    // A heading per month when newest-first; size order has no months.
    let mut rows: Vec<AnyElement> = Vec::with_capacity(shown.len() + 12);
    let mut last_month = None;
    for entry in &shown {
        if sort == Sort::Newest {
            let month = entry
                .message
                .timestamp
                .with_timezone(&chrono::Local)
                .format("%B %Y")
                .to_string();
            if last_month.as_ref() != Some(&month) {
                rows.push(
                    div()
                        .pt(metrics.space_sm())
                        .text_size(metrics.text_micro())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(cx.theme().muted_foreground)
                        .child(month.clone())
                        .into_any_element(),
                );
                last_month = Some(month);
            }
        }
        rows.push(render_row(
            app,
            entry,
            &chat.jid,
            entity.clone(),
            metrics,
            cx,
        ));
    }

    let sort_entity = entity.clone();
    div()
        .w(metrics.details_width())
        .flex_shrink_0()
        .h_full()
        .flex()
        .flex_col()
        .gap(metrics.space_md())
        .p(metrics.space_lg())
        .border_l_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().sidebar)
        .text_color(cx.theme().foreground)
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(metrics.text_heading())
                                .child("Shared files"),
                        )
                        .child(
                            div()
                                .text_size(metrics.text_small())
                                .text_color(cx.theme().muted_foreground)
                                .truncate()
                                .child(chat.name.clone()),
                        ),
                )
                .child(
                    Button::new("close-shared-files")
                        .ghost()
                        .icon(IconName::Close)
                        .tooltip("Close shared files")
                        .on_click(move |_, _, cx| {
                            close.update(cx, |app, cx| {
                                app.shared_files_open = false;
                                cx.notify();
                            })
                        }),
                ),
        )
        .children(
            app.shared_files_search
                .as_ref()
                .map(|input| Input::new(input).cleanable(true)),
        )
        .child(
            div()
                .id("files-tabs")
                .flex()
                .overflow_x_scroll()
                .border_b_1()
                .border_color(cx.theme().border)
                .children(tabs),
        )
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_size(metrics.text_small())
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                    "{} file{}",
                    shown.len(),
                    if shown.len() == 1 { "" } else { "s" }
                ))
                .child(
                    Button::new("shared-files-sort")
                        .ghost()
                        .small()
                        .label(match sort {
                            Sort::Newest => "Newest first",
                            Sort::Largest => "Largest first",
                        })
                        .tooltip("Change order")
                        .on_click(move |_, _, cx| {
                            sort_entity.update(cx, |app, cx| {
                                app.shared_files_sort = match app.shared_files_sort {
                                    Sort::Newest => Sort::Largest,
                                    Sort::Largest => Sort::Newest,
                                };
                                cx.notify();
                            })
                        }),
                ),
        )
        .when(empty, |pane| {
            pane.child(
                div()
                    .text_size(metrics.text_small())
                    .text_color(cx.theme().muted_foreground)
                    .child(if query.is_empty() {
                        "No files in the loaded messages yet."
                    } else {
                        "No files match that search in the loaded messages."
                    }),
            )
        })
        .child(
            div()
                .id("shared-files-list")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(metrics.space_xs())
                .children(rows)
                .when(can_load_older, |list| {
                    list.child(
                        Button::new("shared-load-older")
                            .ghost()
                            .label("Load older messages")
                            .on_click(move |_, _, cx| {
                                more.update(cx, |app, cx| app.load_older_shared_files(&jid, cx))
                            }),
                    )
                }),
        )
}

fn render_row(
    app: &WhatsAppApp,
    entry: &Entry<'_>,
    chat_jid: &str,
    entity: Entity<WhatsAppApp>,
    metrics: Metrics,
    cx: &App,
) -> AnyElement {
    let message = entry.message;
    let saved = app.saved_document_path(&message.id).is_some();
    let downloading = app.is_document_downloading(&message.id) || app.is_downloading(&message.id);
    let is_photo = entry.kind == MediaType::Image;
    let stamp = message
        .timestamp
        .with_timezone(&chrono::Local)
        .format("%b %-d")
        .to_string();
    let detail = match format_size(entry.bytes) {
        Some(size) => format!("{size} · {stamp}"),
        None => stamp,
    };
    let open_id = message.id.clone();
    let open_entity = entity.clone();
    let open_jid = chat_jid.to_string();
    let action_id = message.id.clone();
    let action_jid = chat_jid.to_string();

    div()
        .id(SharedString::from(format!("shared-{}", message.id)))
        .flex()
        .items_center()
        .gap(metrics.space_md())
        .rounded(metrics.radius_md())
        .px(metrics.space_sm())
        .py(metrics.space_sm())
        .hover(|row| row.bg(cx.theme().secondary))
        .when(is_photo, |row| {
            // A photo opens in the viewer: a look, not a save.
            row.cursor_pointer().on_click(move |_, window, cx| {
                open_entity.update(cx, |app, cx| {
                    if app
                        .selected_chat_data()
                        .is_some_and(|chat| chat.jid == open_jid)
                    {
                        app.open_media_viewer(&open_id, window, cx);
                    }
                })
            })
        })
        .child({
            // The badge: a thumbnail where the file is a picture, otherwise
            // what kind of file it is, before the name says so.
            let thumb = matches!(
                entry.kind,
                MediaType::Image | MediaType::Video | MediaType::Sticker
            )
            // Only a picture that is actually here: a photo not yet
            // downloaded has no bytes, and drawing its "thumbnail" left an
            // empty tile with no icon.
            && !entry.media.data.is_empty();
            let thumb = thumb
                .then(|| app.get_decoded_image(&message.id, entry.media, cx))
                .flatten();
            let size = metrics.avatar_row();
            match thumb {
                // The picture itself, cropped to the tile's rounded corners:
                // the image carries the radius, because a parent's overflow
                // clip does not round what is drawn inside it.
                Some(image) => div().flex_shrink_0().size(size).child(
                    img(image.source(false))
                        .size(size)
                        .rounded(size * 0.22)
                        .object_fit(ObjectFit::Cover),
                ),
                None => {
                    let kind = match entry.kind {
                        MediaType::Document => FileKind::Document,
                        MediaType::Image => FileKind::Image,
                        MediaType::Video => FileKind::Video,
                        MediaType::Audio => FileKind::Audio,
                        MediaType::Sticker => FileKind::Sticker,
                    };
                    let label = extension_label(&entry.name, &entry.media.mime_type);
                    file_badge(kind, label.as_deref(), size, cx)
                }
            }
        })
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    div()
                        .truncate()
                        .text_size(metrics.text_secondary())
                        .font_weight(FontWeight::MEDIUM)
                        .child(entry.name.clone()),
                )
                .child(
                    div()
                        .text_size(metrics.text_small())
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .child(
            Button::new(SharedString::from(format!("shared-action-{}", message.id)))
                .ghost()
                .small()
                .icon(if saved {
                    IconName::FolderOpen
                } else {
                    IconName::ArrowDown
                })
                .tooltip(if saved {
                    "Show in folder"
                } else {
                    "Save to Downloads"
                })
                .disabled(downloading)
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    entity.update(cx, |app, cx| {
                        if app
                            .selected_chat_data()
                            .is_some_and(|chat| chat.jid == action_jid)
                        {
                            if saved {
                                app.reveal_saved_document(&action_id, cx);
                            } else {
                                app.save_media(&action_id, cx);
                            }
                        }
                    })
                }),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    // Named rather than globbed: the module's `gpui::*` import carries a
    // `test` macro of its own that shadows the standard one.
    use super::{Entry, badge_label, display_name, format_size, kind_of, matches, size_of};
    use std::sync::Arc;
    use velo_core::{ChatMessage, MediaContent, MediaType};

    fn doc(name: &str, mime: &str) -> MediaContent {
        let mut media = MediaContent::image(Arc::new(vec![1; 10]), mime.into(), false);
        media.media_type = MediaType::Document;
        media.file_name = Some(name.into());
        media
    }

    fn entry<'a>(message: &'a ChatMessage) -> Entry<'a> {
        let media = message.media.as_ref().unwrap();
        Entry {
            message,
            media,
            kind: kind_of(media),
            name: display_name(media),
            bytes: size_of(media),
        }
    }

    fn message(media: MediaContent) -> ChatMessage {
        let mut message =
            ChatMessage::new_incoming("m".into(), "a@s.whatsapp.net".into(), String::new());
        message.media = Some(media);
        message
    }

    #[test]
    fn badges_name_the_file_type() {
        let pdf = message(doc("Invoice March.pdf", "application/pdf"));
        assert_eq!(badge_label(&entry(&pdf)), "PDF");
        let nameless = message(doc("report", "application/vnd.ms-excel"));
        assert_eq!(badge_label(&entry(&nameless)), "XLS");
        let modern = message(doc(
            "notes",
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        ));
        assert_eq!(badge_label(&entry(&modern)), "DOCX");
    }

    #[test]
    fn search_finds_names_captions_and_types() {
        let mut media = doc("Invoice March.pdf", "application/pdf");
        media.caption = Some("Rent for the flat".into());
        let msg = message(media);
        let e = entry(&msg);
        assert!(matches(&e, ""));
        assert!(matches(&e, "invoice"));
        assert!(matches(&e, "rent"));
        assert!(matches(&e, "pdf"));
        assert!(!matches(&e, "holiday"));
    }

    #[test]
    fn sizes_read_like_a_file_manager() {
        assert_eq!(format_size(0), None);
        assert_eq!(format_size(512).as_deref(), Some("512 B"));
        assert_eq!(format_size(1536).as_deref(), Some("1.5 KB"));
        assert_eq!(format_size(5 * 1024 * 1024).as_deref(), Some("5.0 MB"));
    }
}

//! The tile that says what a file is, before its name does.
//!
//! One component for every place a file is listed — a document in a
//! conversation, a row in Shared files — so a PDF looks like the same PDF
//! wherever it appears. Documents get a solid tile in their family's colour
//! with the extension in white, the way the desktop messengers draw them;
//! everything else gets its kind's icon on a soft tint.

use gpui::{App, Div, Hsla, ParentElement as _, Pixels, Styled as _, div};
use gpui_component::{ActiveTheme as _, Icon};

use crate::components::ProductIcon;

/// What a file is, as far as its badge cares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    Document,
    Image,
    Video,
    Audio,
    Sticker,
}

/// The short, upper-case extension shown on a document's tile, or `None`
/// when there is nothing sensible to show (no extension, or a long one).
pub fn extension_label(name: &str, mime: &str) -> Option<String> {
    let from_name = name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty() && ext.len() <= 4 && ext.chars().all(char::is_alphanumeric));
    let from_mime = [
        ("spreadsheetml", "xlsx"),
        ("ms-excel", "xls"),
        ("wordprocessingml", "docx"),
        ("msword", "doc"),
        ("presentationml", "pptx"),
        ("ms-powerpoint", "ppt"),
        ("pdf", "pdf"),
        ("zip", "zip"),
        ("text/plain", "txt"),
        ("csv", "csv"),
    ]
    .into_iter()
    .find(|(needle, _)| mime.contains(needle))
    .map(|(_, short)| short);
    from_name.or(from_mime).map(|ext| ext.to_uppercase())
}

/// The colour a document's tile is filled with: red for PDF, blue for text
/// documents, green for spreadsheets, amber for slides, grey otherwise. From
/// the theme's semantic tokens, so every preset agrees with itself.
pub fn document_colour(label: Option<&str>, cx: &App) -> Hsla {
    let theme = cx.theme();
    match label {
        Some("PDF") => theme.danger,
        Some("DOC" | "DOCX" | "ODT" | "RTF" | "TXT") => theme.info,
        Some("XLS" | "XLSX" | "CSV" | "ODS") => theme.success,
        Some("PPT" | "PPTX" | "KEY" | "ODP") => theme.warning,
        _ => theme.muted_foreground,
    }
}

/// The badge, `size` square. `label` is the document's extension label;
/// ignored for other kinds.
pub fn file_badge(kind: FileKind, label: Option<&str>, size: Pixels, cx: &App) -> Div {
    let theme = cx.theme();
    let radius = size * 0.22;
    let tile = div()
        .flex_shrink_0()
        .size(size)
        .rounded(radius)
        .flex()
        .items_center()
        .justify_center();
    match kind {
        FileKind::Document => {
            let fill = document_colour(label, cx);
            let ink = gpui::white();
            match label {
                Some(label) => tile
                    .bg(fill)
                    .text_size(size * 0.28)
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(ink)
                    .child(label.to_string()),
                None => tile.bg(fill).child(
                    Icon::new(ProductIcon::FileText)
                        .size(size * 0.5)
                        .text_color(ink),
                ),
            }
        }
        other => {
            let (icon, colour) = match other {
                FileKind::Image => (ProductIcon::Image, theme.success),
                FileKind::Video => (ProductIcon::Film, theme.info),
                FileKind::Audio => (ProductIcon::Volume, theme.warning),
                _ => (ProductIcon::Sticker, theme.primary),
            };
            tile.bg(colour.opacity(0.16))
                .child(Icon::new(icon).size(size * 0.5).text_color(colour))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extension_label;

    #[test]
    fn labels_come_from_the_name_then_the_type() {
        assert_eq!(
            extension_label("Invoice.pdf", "application/pdf").as_deref(),
            Some("PDF")
        );
        assert_eq!(
            extension_label("report", "application/vnd.ms-excel").as_deref(),
            Some("XLS")
        );
        assert_eq!(
            extension_label(
                "notes",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
            )
            .as_deref(),
            Some("DOCX")
        );
        assert_eq!(
            extension_label("archive.backup", "application/octet-stream"),
            None
        );
    }
}

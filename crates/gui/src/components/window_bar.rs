//! The window's title strip, drawn by the app instead of by Windows.
//!
//! The system caption is a band of its own colour with a hairline under it,
//! which is what made the window read as a web page in a frame. This strip
//! is painted in the panels' own colour with no rule beneath it, so the
//! sidebar and the conversation run up to the window's top edge the way a
//! Mac app's do. Dragging, double-click to maximise, Snap layouts and the
//! three buttons keep their native behaviour: the strip only marks *where*
//! they are (`WindowControlArea`), and Windows does the rest.
//!
//! Not drawn on the web, where the browser owns the frame.

use gpui::{App, IntoElement, ParentElement, Styled, div};
use gpui_component::{ActiveTheme as _, Icon, TitleBar, h_flex};

use crate::components::{ProductIcon, parts};
use crate::theme::ActiveProductTheme as _;

pub fn render_window_bar(cx: &App) -> impl IntoElement + use<> {
    let metrics = cx.product().metrics;
    TitleBar::new()
        // The panels' colour, so the strip is part of them rather than a
        // frame around them, and no hairline under it to say otherwise.
        .bg(cx.theme().sidebar)
        .border_b_0()
        .child(
            h_flex()
                .gap(metrics.space_sm())
                .items_center()
                .child(
                    // Tinted with the accent, so the mark follows the colour
                    // chosen in Appearance.
                    Icon::from(ProductIcon::VeloMark)
                        .size(metrics.icon_small())
                        .text_color(cx.theme().primary),
                )
                .child(
                    div()
                        .text_size(metrics.text_small())
                        .text_color(parts::subtle(cx))
                        .child("Velo"),
                ),
        )
}

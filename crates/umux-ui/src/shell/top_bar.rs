// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{Div, div, prelude::*, px};
use umux_ui_kit::{MUTED_TEXT, PANEL};

pub fn top_bar(title: String) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(40.0))
        .px(px(14.0))
        .bg(PANEL)
        .text_size(px(12.0))
        .child(div().font_weight(gpui::FontWeight::BOLD).child("umux"))
        .child(div().text_color(MUTED_TEXT).child(title))
}

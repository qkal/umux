// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{Div, div, prelude::*, px};
use umux_ui_kit::{BORDER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE};

use crate::view_model::SurfaceTab;

pub fn surface_tabs(tabs: Vec<SurfaceTab>) -> Div {
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(34.0))
        .bg(PANEL)
        .border_b_1()
        .border_color(BORDER)
        .children(tabs.into_iter().map(surface_tab))
}

fn surface_tab(tab: SurfaceTab) -> Div {
    div()
        .flex()
        .items_center()
        .h_full()
        .px(px(12.0))
        .text_size(px(12.0))
        .text_color(if tab.selected { TEXT } else { MUTED_TEXT })
        .when(tab.selected, |tab| tab.bg(BORDER))
        .child(tab.label)
        .when(tab.unread, |tab| {
            tab.child(
                div()
                    .ml(px(6.0))
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(UNREAD_BLUE),
            )
        })
}

// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{Div, div, prelude::*, px};
use umux_ui_kit::{BORDER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE};

use crate::view_model::WorkspaceRow;

pub fn workspace_rail(rows: Vec<WorkspaceRow>) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .w(px(180.0))
        .h_full()
        .p(px(8.0))
        .bg(PANEL)
        .border_r_1()
        .border_color(BORDER)
        .children(rows.into_iter().map(workspace_row))
}

fn workspace_row(row: WorkspaceRow) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .min_h(px(30.0))
        .px(px(8.0))
        .text_size(px(12.0))
        .text_color(if row.selected { TEXT } else { MUTED_TEXT })
        .when(row.selected, |row| row.bg(BORDER))
        .child(row.label)
        .when(row.unread, |row| {
            row.child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(UNREAD_BLUE))
        })
}

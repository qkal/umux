// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Div, MouseButton, div, prelude::*, px};
use umux_core::WorkspaceId;
use umux_ui_kit::{BORDER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE};

use crate::view_model::WorkspaceRow;

pub fn workspace_rail(
    rows: Vec<WorkspaceRow>,
    on_select: impl Fn(WorkspaceId, &mut App) + Clone + 'static,
    on_new: impl Fn(&mut App) + Clone + 'static,
) -> Div {
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
        .children(
            rows.into_iter()
                .map(move |row| workspace_row(row, on_select.clone())),
        )
        .child(div().flex_1())
        .child(new_workspace_control(on_new))
}

fn workspace_row(
    row: WorkspaceRow,
    on_select: impl Fn(WorkspaceId, &mut App) + Clone + 'static,
) -> Div {
    let id = row.id;

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
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(id, cx))
        .child(row.label)
        .when(row.unread, |row| {
            row.child(div().w(px(6.0)).h(px(6.0)).rounded_full().bg(UNREAD_BLUE))
        })
}

fn new_workspace_control(on_new: impl Fn(&mut App) + Clone + 'static) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .min_h(px(28.0))
        .px(px(8.0))
        .text_size(px(12.0))
        .text_color(MUTED_TEXT)
        .border_1()
        .border_color(BORDER)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_new(cx))
        .child("+ ws")
}

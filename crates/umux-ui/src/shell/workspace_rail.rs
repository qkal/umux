// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Div, MouseButton, div, prelude::*, px};
use umux_core::WorkspaceId;
use umux_ui_kit::{
    ACCENT, ACTIVE, BORDER, DIM_TEXT, ELEVATED, HOVER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE,
};

use crate::view_model::WorkspaceRow;

pub(crate) const RAIL_LABEL: &str = "WORKSPACES";
pub(crate) const NEW_WORKSPACE_LABEL: &str = "+";
pub(crate) const WORKSPACE_ROW_HEIGHT: f32 = 30.0;

pub fn workspace_rail(
    rows: Vec<WorkspaceRow>,
    on_select: impl Fn(WorkspaceId, &mut App) + Clone + 'static,
    on_new: impl Fn(&mut App) + Clone + 'static,
) -> Div {
    div()
        .flex()
        .flex_col()
        .flex_none()
        .w(px(178.0))
        .h_full()
        .p(px(8.0))
        .bg(PANEL)
        .border_r_1()
        .border_color(BORDER)
        .child(
            div()
                .h(px(24.0))
                .px(px(8.0))
                .flex()
                .items_center()
                .text_size(px(10.0))
                .text_color(DIM_TEXT)
                .child(RAIL_LABEL),
        )
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
) -> impl IntoElement {
    let id = row.id;

    div()
        .id(("workspace-row", id.0))
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(WORKSPACE_ROW_HEIGHT))
        .mt(px(2.0))
        .px(px(8.0))
        .border_1()
        .border_color(if row.selected { ACCENT } else { PANEL })
        .bg(if row.selected { ELEVATED } else { PANEL })
        .text_size(px(12.0))
        .text_color(if row.selected { TEXT } else { MUTED_TEXT })
        .cursor_pointer()
        .hover(|style| style.bg(HOVER).text_color(TEXT))
        .active(|style| style.bg(ACTIVE))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(id, cx))
        .child(
            div()
                .flex()
                .items_center()
                .min_w(px(0.0))
                .child(
                    div()
                        .w(px(8.0))
                        .text_color(if row.selected { ACCENT } else { DIM_TEXT })
                        .child(if row.selected { ">" } else { "" }),
                )
                .child(div().ml(px(6.0)).min_w(px(0.0)).truncate().child(row.label)),
        )
        .child(unread_marker(row.unread))
}

fn unread_marker(unread: bool) -> Div {
    div()
        .ml(px(8.0))
        .w(px(6.0))
        .h(px(6.0))
        .when(unread, |marker| marker.rounded_full().bg(UNREAD_BLUE))
}

fn new_workspace_control(on_new: impl Fn(&mut App) + Clone + 'static) -> impl IntoElement {
    div()
        .id("new-workspace")
        .flex()
        .items_center()
        .justify_center()
        .w_full()
        .h(px(28.0))
        .mt(px(8.0))
        .border_1()
        .border_color(BORDER)
        .bg(ELEVATED)
        .text_size(px(13.0))
        .text_color(MUTED_TEXT)
        .cursor_pointer()
        .hover(|style| style.bg(HOVER).text_color(TEXT))
        .active(|style| style.bg(ACTIVE))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_new(cx))
        .child(NEW_WORKSPACE_LABEL)
}

#[cfg(test)]
mod tests {
    use super::{NEW_WORKSPACE_LABEL, RAIL_LABEL, WORKSPACE_ROW_HEIGHT};

    #[test]
    fn workspace_rail_uses_compact_ascii_labels() {
        assert_eq!(RAIL_LABEL, "WORKSPACES");
        assert_eq!(NEW_WORKSPACE_LABEL, "+");
    }

    #[test]
    fn workspace_rows_have_stable_height() {
        assert_eq!(WORKSPACE_ROW_HEIGHT, 30.0);
    }
}

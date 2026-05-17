// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{Div, div, prelude::*, px};
use umux_core::PaneId;
use umux_core::model::{Pane, SplitAxis, SplitTree, SurfaceKind, Workspace};
use umux_ui_kit::{BACKGROUND, BORDER, MUTED_TEXT, PANEL, TEXT};

use crate::shell::{surface_tabs, unsupported_surface_message};
use crate::view_model;

pub fn pane_group(workspace: &Workspace) -> Div {
    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .h_full()
        .bg(BACKGROUND)
        .child(layout_node(&workspace.layout, workspace))
}

fn layout_node(layout: &SplitTree, workspace: &Workspace) -> Div {
    match layout {
        SplitTree::Leaf(pane_id) => pane_slot(*pane_id, workspace),
        SplitTree::Split {
            axis,
            first,
            second,
        } => div()
            .flex()
            .flex_1()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .when(layout_axis_is_row(*axis), |node| node.flex_row())
            .when(!layout_axis_is_row(*axis), |node| node.flex_col())
            .child(pane_slot(*first, workspace))
            .child(pane_slot(*second, workspace)),
    }
}

fn pane_slot(pane_id: PaneId, workspace: &Workspace) -> Div {
    workspace
        .pane(pane_id)
        .map(|pane| pane_view(pane, workspace))
        .unwrap_or_else(|| missing_pane_view(pane_id))
}

fn pane_view(pane: &Pane, workspace: &Workspace) -> Div {
    let view = view_model::pane_view(pane, workspace.selected_pane);
    let selected_surface = pane.surface(pane.selected_surface);
    let body = selected_surface
        .map(|surface| match surface.kind {
            SurfaceKind::Terminal => "terminal surface".to_string(),
            kind => unsupported_surface_message(kind, &surface.title),
        })
        .unwrap_or_else(|| "missing selected surface".to_string());

    div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .h_full()
        .border_l_1()
        .border_color(BORDER)
        .when(view.selected, |pane| pane.bg(PANEL))
        .child(surface_tabs(view.tabs))
        .child(
            div()
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .p(px(16.0))
                .text_size(px(13.0))
                .text_color(if view.selected { TEXT } else { MUTED_TEXT })
                .child(body),
        )
}

fn missing_pane_view(pane_id: PaneId) -> Div {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .border_1()
        .border_color(BORDER)
        .text_size(px(13.0))
        .text_color(MUTED_TEXT)
        .child(format!("missing pane {}", pane_id.0))
}

fn layout_axis_is_row(axis: SplitAxis) -> bool {
    matches!(axis, SplitAxis::Vertical)
}

#[cfg(test)]
fn pane_ids_in_layout(layout: &SplitTree) -> Vec<PaneId> {
    match layout {
        SplitTree::Leaf(pane_id) => vec![*pane_id],
        SplitTree::Split { first, second, .. } => vec![*first, *second],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_core::AppModel;

    #[test]
    fn vertical_split_renders_left_to_right_in_layout_order() {
        let mut model = AppModel::new("C:/work/alpha");
        let first = model.selected_pane().unwrap().id;
        let second = model.split_selected_pane(SplitAxis::Vertical).unwrap();
        let workspace = model.selected_workspace().unwrap();

        assert!(matches!(
            workspace.layout,
            SplitTree::Split {
                axis: SplitAxis::Vertical,
                ..
            }
        ));
        assert!(layout_axis_is_row(SplitAxis::Vertical));
        assert_eq!(pane_ids_in_layout(&workspace.layout), vec![first, second]);
    }

    #[test]
    fn horizontal_split_renders_top_to_bottom_in_layout_order() {
        let mut model = AppModel::new("C:/work/alpha");
        let first = model.selected_pane().unwrap().id;
        let second = model.split_selected_pane(SplitAxis::Horizontal).unwrap();
        let workspace = model.selected_workspace().unwrap();

        assert!(matches!(
            workspace.layout,
            SplitTree::Split {
                axis: SplitAxis::Horizontal,
                ..
            }
        ));
        assert!(!layout_axis_is_row(SplitAxis::Horizontal));
        assert_eq!(pane_ids_in_layout(&workspace.layout), vec![first, second]);
    }
}

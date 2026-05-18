// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{Div, IntoElement, div, prelude::*, px};
use umux_app::AppController;
use umux_core::PaneId;
use umux_core::model::{Pane, SplitAxis, SplitTree, SurfaceKind, Workspace};
use umux_ui_kit::{BACKGROUND, BORDER, MUTED_TEXT, PANEL, TEXT};

use crate::shell::{surface_tabs, unsupported_surface_message};
use crate::terminal::{TerminalSurfaceState, terminal_surface};
use crate::view_model;

pub fn pane_group(
    controller: &AppController,
    workspace: &Workspace,
    terminal_surface_state: &TerminalSurfaceState,
) -> Div {
    div()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .h_full()
        .bg(BACKGROUND)
        .child(layout_node(
            &workspace.layout,
            controller,
            workspace,
            terminal_surface_state,
        ))
}

fn layout_node(
    layout: &SplitTree,
    controller: &AppController,
    workspace: &Workspace,
    terminal_surface_state: &TerminalSurfaceState,
) -> Div {
    match layout {
        SplitTree::Leaf(pane_id) => {
            pane_slot(*pane_id, controller, workspace, terminal_surface_state)
        }
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
            .child(pane_slot(
                *first,
                controller,
                workspace,
                terminal_surface_state,
            ))
            .child(pane_slot(
                *second,
                controller,
                workspace,
                terminal_surface_state,
            )),
    }
}

fn pane_slot(
    pane_id: PaneId,
    controller: &AppController,
    workspace: &Workspace,
    terminal_surface_state: &TerminalSurfaceState,
) -> Div {
    workspace
        .pane(pane_id)
        .map(|pane| pane_view(pane, controller, workspace, terminal_surface_state))
        .unwrap_or_else(|| missing_pane_view(pane_id))
}

fn pane_view(
    pane: &Pane,
    controller: &AppController,
    workspace: &Workspace,
    terminal_surface_state: &TerminalSurfaceState,
) -> Div {
    let view = view_model::pane_view(pane, workspace.selected_pane);
    let selected_surface = pane.surface(pane.selected_surface);
    let body = selected_surface
        .map(|surface| match surface.kind {
            SurfaceKind::Terminal => {
                terminal_surface(controller, surface.id, terminal_surface_state).into_any_element()
            }
            kind => unsupported_body(
                unsupported_surface_message(kind, &surface.title),
                view.selected,
            ),
        })
        .unwrap_or_else(|| unsupported_body("missing selected surface".to_string(), view.selected));

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
        .child(body)
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

fn unsupported_body(body: String, selected: bool) -> gpui::AnyElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .p(px(16.0))
        .text_size(px(13.0))
        .text_color(if selected { TEXT } else { MUTED_TEXT })
        .child(body)
        .into_any_element()
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

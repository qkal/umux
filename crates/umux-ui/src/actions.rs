// SPDX-License-Identifier: GPL-3.0-or-later

use umux_app::AppAction;
use umux_core::{AppModel, SplitAxis};

gpui::actions!(
    umux,
    [
        NewWorkspace,
        NewTerminalTab,
        CloseTerminalTab,
        CloseWorkspace,
        SplitRight,
        SplitDown,
        JumpLatestUnread,
    ]
);

pub fn close_surface_action(model: &AppModel) -> Option<AppAction> {
    model
        .selected_pane()
        .ok()
        .map(|pane| AppAction::CloseSurface(pane.selected_surface))
}

pub fn close_workspace_action(model: &AppModel) -> Option<AppAction> {
    model
        .selected_workspace()
        .ok()
        .map(|workspace| AppAction::CloseWorkspace(workspace.id))
}

pub fn split_right_action() -> AppAction {
    AppAction::SplitPane(SplitAxis::Vertical)
}

pub fn split_down_action() -> AppAction {
    AppAction::SplitPane(SplitAxis::Horizontal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_surface_action_targets_selected_pane_selected_surface() {
        let mut model = AppModel::new("C:/work/alpha");
        let first = model.selected_pane().unwrap().selected_surface;
        let second = model.open_terminal_surface().unwrap();

        assert_ne!(first, second);
        assert_eq!(
            close_surface_action(&model),
            Some(AppAction::CloseSurface(second))
        );
    }

    #[test]
    fn close_workspace_action_targets_selected_workspace() {
        let mut model = AppModel::new("C:/work/alpha");
        let workspace = model
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();

        assert_eq!(
            close_workspace_action(&model),
            Some(AppAction::CloseWorkspace(workspace))
        );
    }

    #[test]
    fn split_actions_map_to_gpui_directions() {
        assert_eq!(
            split_right_action(),
            AppAction::SplitPane(SplitAxis::Vertical)
        );
        assert_eq!(
            split_down_action(),
            AppAction::SplitPane(SplitAxis::Horizontal)
        );
    }
}

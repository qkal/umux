// SPDX-License-Identifier: GPL-3.0-or-later

use umux_core::model::{Pane, Surface, SurfaceKind, Workspace};
use umux_core::{PaneId, SurfaceId, WorkspaceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceRow {
    pub id: WorkspaceId,
    pub label: String,
    pub selected: bool,
    pub unread: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SurfaceTab {
    pub id: SurfaceId,
    pub label: String,
    pub selected: bool,
    pub unread: bool,
    pub kind: SurfaceKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PaneView {
    pub id: PaneId,
    pub selected: bool,
    pub tabs: Vec<SurfaceTab>,
}

pub fn workspace_label(workspace: &Workspace) -> String {
    if workspace.unread {
        format!("{} *", workspace.title)
    } else {
        workspace.title.clone()
    }
}

pub fn surface_label(surface: &Surface) -> String {
    if surface.unread {
        format!("{} *", surface.title)
    } else {
        surface.title.clone()
    }
}

pub fn workspace_rows(workspaces: &[Workspace], selected: WorkspaceId) -> Vec<WorkspaceRow> {
    workspaces
        .iter()
        .map(|workspace| WorkspaceRow {
            id: workspace.id,
            label: workspace_label(workspace),
            selected: workspace.id == selected,
            unread: workspace.unread,
        })
        .collect()
}

pub fn pane_view(pane: &Pane, selected_pane: PaneId) -> PaneView {
    PaneView {
        id: pane.id,
        selected: pane.id == selected_pane,
        tabs: pane
            .surfaces
            .iter()
            .map(|surface| SurfaceTab {
                id: surface.id,
                label: surface_label(surface),
                selected: surface.id == pane.selected_surface,
                unread: surface.unread,
                kind: surface.kind,
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_core::AppModel;
    use umux_core::model::SurfaceKind;

    #[test]
    fn workspace_row_marks_unread() {
        let mut model = AppModel::new("C:/work/alpha");
        let surface_id = model.selected_pane().unwrap().selected_surface;
        model
            .mark_surface_unread(surface_id, "done".to_string())
            .unwrap();
        let window = model.selected_window().unwrap();

        let rows = workspace_rows(&window.workspaces, window.selected_workspace);

        assert_eq!(rows[0].label, "alpha *");
        assert!(rows[0].selected);
        assert!(rows[0].unread);
    }

    #[test]
    fn pane_view_preserves_surface_kind() {
        let mut model = AppModel::new("C:/work/alpha");
        let browser = model
            .open_browser_surface("https://example.com".to_string())
            .unwrap();
        let pane = model.selected_pane().unwrap();

        let view = pane_view(pane, pane.id);

        assert_eq!(view.id, pane.id);
        assert!(view.selected);
        assert_eq!(view.tabs.last().unwrap().id, browser);
        assert_eq!(view.tabs.last().unwrap().kind, SurfaceKind::Browser);
    }
}

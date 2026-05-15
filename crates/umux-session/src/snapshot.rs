// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use thiserror::Error;
use umux_core::{
    AppModel, SplitTree, SurfaceKind,
    ids::{PaneId, SurfaceId, WindowId, WorkspaceId},
    model::{AppWindow, Pane, Surface, Workspace},
};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("failed to serialize or deserialize session snapshot: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppSnapshot {
    pub schema_version: u32,
    pub selected_window: WindowId,
    pub windows: Vec<AppWindowSnapshot>,
}

impl AppSnapshot {
    pub fn from_model(app: &AppModel) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            selected_window: app.selected_window,
            windows: app.windows.iter().map(AppWindowSnapshot::from).collect(),
        }
    }

    pub fn to_json_string(&self) -> Result<String, SessionError> {
        serde_json::to_string_pretty(self).map_err(SessionError::from)
    }

    pub fn from_json_str(json: &str) -> Result<Self, SessionError> {
        serde_json::from_str(json).map_err(SessionError::from)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppWindowSnapshot {
    pub id: WindowId,
    pub selected_workspace: WorkspaceId,
    pub workspaces: Vec<WorkspaceSnapshot>,
}

impl From<&AppWindow> for AppWindowSnapshot {
    fn from(window: &AppWindow) -> Self {
        Self {
            id: window.id,
            selected_workspace: window.selected_workspace,
            workspaces: window
                .workspaces
                .iter()
                .map(WorkspaceSnapshot::from)
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkspaceSnapshot {
    pub id: WorkspaceId,
    pub title: String,
    pub cwd: String,
    pub selected_pane: PaneId,
    pub layout: SplitTree,
    pub unread: bool,
    pub latest_notification: Option<String>,
    pub panes: Vec<PaneSnapshot>,
}

impl From<&Workspace> for WorkspaceSnapshot {
    fn from(workspace: &Workspace) -> Self {
        Self {
            id: workspace.id,
            title: workspace.title.clone(),
            cwd: workspace.cwd.clone(),
            selected_pane: workspace.selected_pane,
            layout: workspace.layout.clone(),
            unread: workspace.unread,
            latest_notification: workspace.latest_notification.clone(),
            panes: workspace.panes.iter().map(PaneSnapshot::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaneSnapshot {
    pub id: PaneId,
    pub cwd: String,
    pub selected_surface: SurfaceId,
    pub surfaces: Vec<SurfaceSnapshot>,
}

impl From<&Pane> for PaneSnapshot {
    fn from(pane: &Pane) -> Self {
        Self {
            id: pane.id,
            cwd: pane.cwd.clone(),
            selected_surface: pane.selected_surface,
            surfaces: pane.surfaces.iter().map(SurfaceSnapshot::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSnapshot {
    pub id: SurfaceId,
    pub kind: SurfaceKind,
    pub title: String,
    pub unread: bool,
}

impl From<&Surface> for SurfaceSnapshot {
    fn from(surface: &Surface) -> Self {
        Self {
            id: surface.id,
            kind: surface.kind,
            title: surface.title.clone(),
            unread: surface.unread,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppSnapshot;
    use umux_core::{AppModel, SplitAxis, SurfaceKind};

    #[test]
    fn snapshot_round_trips_app_owned_state() {
        let mut app = AppModel::new("C:/work/alpha");
        app.split_selected_pane(SplitAxis::Vertical).unwrap();
        app.open_browser_surface("https://example.com".to_string())
            .unwrap();

        let snapshot = AppSnapshot::from_model(&app);
        let json = snapshot.to_json_string().unwrap();
        let snapshot = AppSnapshot::from_json_str(&json).unwrap();

        assert_eq!(snapshot.schema_version, 1);
        assert_eq!(snapshot.windows.len(), 1);
        assert_eq!(snapshot.windows[0].workspaces[0].panes.len(), 2);
        assert!(
            snapshot.windows[0].workspaces[0]
                .panes
                .iter()
                .flat_map(|pane| pane.surfaces.iter())
                .any(|surface| surface.kind == SurfaceKind::Browser)
        );
    }

    #[test]
    fn snapshot_does_not_store_live_process_state() {
        let app = AppModel::new("C:/work/alpha");
        let snapshot = AppSnapshot::from_model(&app);

        let json = snapshot.to_json_string().unwrap();

        assert!(!json.contains("pid"));
        assert!(!json.contains("process_handle"));
        assert!(!json.contains("thread_handle"));
    }
}

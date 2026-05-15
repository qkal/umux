// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use thiserror::Error;
use umux_core::{
    AppModel, SplitAxis as CoreSplitAxis, SplitTree as CoreSplitTree,
    SurfaceKind as CoreSurfaceKind,
    ids::{
        PaneId as CorePaneId, SurfaceId as CoreSurfaceId, WindowId as CoreWindowId,
        WorkspaceId as CoreWorkspaceId,
    },
    model::{AppWindow, Pane, Surface, Workspace},
};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("failed to serialize or deserialize session snapshot: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported session snapshot schema version {0}")]
    UnsupportedSchemaVersion(u32),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppSnapshot {
    pub schema_version: u32,
    pub selected_window: SnapshotWindowId,
    pub windows: Vec<AppWindowSnapshot>,
}

impl AppSnapshot {
    pub fn from_model(app: &AppModel) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            selected_window: app.selected_window.into(),
            windows: app.windows.iter().map(AppWindowSnapshot::from).collect(),
        }
    }

    pub fn to_json_string(&self) -> Result<String, SessionError> {
        serde_json::to_string_pretty(self).map_err(SessionError::from)
    }

    pub fn from_json_str(json: &str) -> Result<Self, SessionError> {
        let envelope: SnapshotEnvelope = serde_json::from_str(json)?;
        if envelope.schema_version != SCHEMA_VERSION {
            return Err(SessionError::UnsupportedSchemaVersion(
                envelope.schema_version,
            ));
        }

        let snapshot: Self = serde_json::from_str(json)?;

        Ok(snapshot)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
struct SnapshotEnvelope {
    schema_version: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SnapshotWindowId(pub u64);

impl From<CoreWindowId> for SnapshotWindowId {
    fn from(id: CoreWindowId) -> Self {
        Self(id.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SnapshotWorkspaceId(pub u64);

impl From<CoreWorkspaceId> for SnapshotWorkspaceId {
    fn from(id: CoreWorkspaceId) -> Self {
        Self(id.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SnapshotPaneId(pub u64);

impl From<CorePaneId> for SnapshotPaneId {
    fn from(id: CorePaneId) -> Self {
        Self(id.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct SnapshotSurfaceId(pub u64);

impl From<CoreSurfaceId> for SnapshotSurfaceId {
    fn from(id: CoreSurfaceId) -> Self {
        Self(id.0)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotSurfaceKind {
    Terminal,
    Browser,
}

impl From<CoreSurfaceKind> for SnapshotSurfaceKind {
    fn from(kind: CoreSurfaceKind) -> Self {
        match kind {
            CoreSurfaceKind::Terminal => Self::Terminal,
            CoreSurfaceKind::Browser => Self::Browser,
        }
    }
}

impl PartialEq<CoreSurfaceKind> for SnapshotSurfaceKind {
    fn eq(&self, other: &CoreSurfaceKind) -> bool {
        *self == Self::from(*other)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SnapshotSplitAxis {
    Horizontal,
    Vertical,
}

impl From<CoreSplitAxis> for SnapshotSplitAxis {
    fn from(axis: CoreSplitAxis) -> Self {
        match axis {
            CoreSplitAxis::Horizontal => Self::Horizontal,
            CoreSplitAxis::Vertical => Self::Vertical,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum SnapshotSplitTree {
    Leaf {
        pane: SnapshotPaneId,
    },
    Split {
        axis: SnapshotSplitAxis,
        first: SnapshotPaneId,
        second: SnapshotPaneId,
    },
}

impl From<&CoreSplitTree> for SnapshotSplitTree {
    fn from(tree: &CoreSplitTree) -> Self {
        match tree {
            CoreSplitTree::Leaf(pane) => Self::Leaf {
                pane: (*pane).into(),
            },
            CoreSplitTree::Split {
                axis,
                first,
                second,
            } => Self::Split {
                axis: (*axis).into(),
                first: (*first).into(),
                second: (*second).into(),
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppWindowSnapshot {
    pub id: SnapshotWindowId,
    pub selected_workspace: SnapshotWorkspaceId,
    pub workspaces: Vec<WorkspaceSnapshot>,
}

impl From<&AppWindow> for AppWindowSnapshot {
    fn from(window: &AppWindow) -> Self {
        Self {
            id: window.id.into(),
            selected_workspace: window.selected_workspace.into(),
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
    pub id: SnapshotWorkspaceId,
    pub title: String,
    pub cwd: String,
    pub selected_pane: SnapshotPaneId,
    pub layout: SnapshotSplitTree,
    pub unread: bool,
    pub latest_notification: Option<String>,
    pub panes: Vec<PaneSnapshot>,
}

impl From<&Workspace> for WorkspaceSnapshot {
    fn from(workspace: &Workspace) -> Self {
        Self {
            id: workspace.id.into(),
            title: workspace.title.clone(),
            cwd: workspace.cwd.clone(),
            selected_pane: workspace.selected_pane.into(),
            layout: (&workspace.layout).into(),
            unread: workspace.unread,
            latest_notification: workspace.latest_notification.clone(),
            panes: workspace.panes.iter().map(PaneSnapshot::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct PaneSnapshot {
    pub id: SnapshotPaneId,
    pub cwd: String,
    pub selected_surface: SnapshotSurfaceId,
    pub surfaces: Vec<SurfaceSnapshot>,
}

impl From<&Pane> for PaneSnapshot {
    fn from(pane: &Pane) -> Self {
        Self {
            id: pane.id.into(),
            cwd: pane.cwd.clone(),
            selected_surface: pane.selected_surface.into(),
            surfaces: pane.surfaces.iter().map(SurfaceSnapshot::from).collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SurfaceSnapshot {
    pub id: SnapshotSurfaceId,
    pub kind: SnapshotSurfaceKind,
    pub title: String,
    pub unread: bool,
}

impl From<&Surface> for SurfaceSnapshot {
    fn from(surface: &Surface) -> Self {
        Self {
            id: surface.id.into(),
            kind: surface.kind.into(),
            title: surface.title.clone(),
            unread: surface.unread,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AppSnapshot, SessionError};
    use serde_json::Value;
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

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let json = r#"{"schema_version":2,"selected_window":1,"windows":[]}"#;

        let result = AppSnapshot::from_json_str(json);

        assert!(matches!(
            result,
            Err(SessionError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn unsupported_schema_version_is_rejected_before_body_deserialization() {
        let json = r#"{"schema_version":2,"future_shape":{"anything":true}}"#;

        let result = AppSnapshot::from_json_str(json);

        assert!(matches!(
            result,
            Err(SessionError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn snapshot_json_uses_stable_v1_wire_contract() {
        let mut app = AppModel::new("C:/work/alpha");
        app.split_selected_pane(SplitAxis::Vertical).unwrap();
        app.open_browser_surface("https://example.com".to_string())
            .unwrap();
        let selected_surface = app.selected_pane().unwrap().selected_surface;
        app.mark_surface_unread(selected_surface, "Browser updated".to_string())
            .unwrap();

        let json = AppSnapshot::from_model(&app).to_json_string().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["selected_window"], 1);

        let window = &value["windows"][0];
        assert_eq!(window["id"], 1);
        assert_eq!(window["selected_workspace"], 2);

        let workspace = &window["workspaces"][0];
        assert_eq!(workspace["id"], 2);
        assert_eq!(workspace["title"], "alpha");
        assert_eq!(workspace["cwd"], "C:/work/alpha");
        assert_eq!(workspace["selected_pane"], 5);
        assert_eq!(workspace["unread"], true);
        assert_eq!(workspace["latest_notification"], "Browser updated");
        assert_eq!(workspace["layout"]["type"], "split");
        assert_eq!(workspace["layout"]["axis"], "vertical");
        assert_eq!(workspace["layout"]["first"], 3);
        assert_eq!(workspace["layout"]["second"], 5);

        let selected_pane = workspace["panes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|pane| pane["id"] == 5)
            .unwrap();
        assert_eq!(selected_pane["cwd"], "C:/work/alpha");
        assert_eq!(selected_pane["selected_surface"], 7);

        let browser_surface = selected_pane["surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .find(|surface| surface["kind"] == "browser")
            .unwrap();
        assert_eq!(browser_surface["id"], 7);
        assert_eq!(browser_surface["title"], "https://example.com");
        assert_eq!(browser_surface["unread"], true);
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use thiserror::Error;
use umux_core::{
    AppModel, SplitAxis as CoreSplitAxis, SplitTree as CoreSplitTree,
    SurfaceKind as CoreSurfaceKind, UnreadTarget as CoreUnreadTarget,
    ids::{
        IdGen, PaneId as CorePaneId, SurfaceId as CoreSurfaceId, WindowId as CoreWindowId,
        WorkspaceId as CoreWorkspaceId,
    },
    model::{AppWindow, Pane, Surface, Workspace},
};

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("failed to serialize or deserialize session snapshot: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported session snapshot schema version {0}")]
    UnsupportedSchemaVersion(u32),
    #[error("restored snapshot selected window is missing")]
    MissingSelectedWindow,
    #[error("restored snapshot window has no workspaces")]
    MissingSelectedWorkspace,
    #[error("restored snapshot workspace has no panes")]
    MissingSelectedPane,
    #[error("restored snapshot pane has no surfaces")]
    MissingSelectedSurface,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppSnapshot {
    pub schema_version: u32,
    pub selected_window: SnapshotWindowId,
    #[serde(default)]
    pub latest_unread_target: Option<SnapshotUnreadTarget>,
    #[serde(default)]
    pub next_unread_sequence: u64,
    pub windows: Vec<AppWindowSnapshot>,
}

impl AppSnapshot {
    pub fn from_model(app: &AppModel) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            selected_window: app.selected_window.into(),
            latest_unread_target: app.latest_unread_target.as_ref().map(Into::into),
            next_unread_sequence: app.next_unread_sequence,
            windows: app.windows.iter().map(AppWindowSnapshot::from).collect(),
        }
    }

    pub fn to_json_string(&self) -> Result<String, SessionError> {
        serde_json::to_string_pretty(self).map_err(SessionError::from)
    }

    pub fn from_json_str(json: &str) -> Result<Self, SessionError> {
        let envelope: SnapshotEnvelope = serde_json::from_str(json)?;
        match envelope.schema_version {
            SCHEMA_VERSION => serde_json::from_str(json).map_err(SessionError::from),
            1 => {
                let mut snapshot: Self = serde_json::from_str(json)?;
                snapshot.schema_version = SCHEMA_VERSION;
                snapshot.latest_unread_target = None;
                snapshot.next_unread_sequence = 1;
                Ok(snapshot)
            }
            version => Err(SessionError::UnsupportedSchemaVersion(version)),
        }
    }

    pub fn into_model(self) -> Result<AppModel, SessionError> {
        let mut max_id = self.max_restored_id();
        if self.windows.is_empty() {
            return Err(SessionError::MissingSelectedWindow);
        }

        let snapshot_latest_unread_target = self.latest_unread_target.map(Into::into);
        let mut max_unread_sequence = snapshot_latest_unread_target
            .as_ref()
            .map(|target: &CoreUnreadTarget| target.sequence)
            .unwrap_or(0);
        let selected_window_id = self.selected_window.into();

        let mut windows = Vec::with_capacity(self.windows.len());
        for window in self.windows {
            let mut workspaces = Vec::with_capacity(window.workspaces.len());
            for workspace in window.workspaces {
                let mut panes = Vec::with_capacity(workspace.panes.len());
                for pane in workspace.panes {
                    let mut surfaces = pane
                        .surfaces
                        .into_iter()
                        .map(|surface| {
                            max_unread_sequence =
                                max_unread_sequence.max(surface.unread_sequence.unwrap_or(0));
                            Surface::from(surface)
                        })
                        .collect::<Vec<_>>();

                    if surfaces.is_empty() {
                        return Err(SessionError::MissingSelectedSurface);
                    }

                    let selected_surface = reconcile_selected_surface(
                        pane.selected_surface.into(),
                        &mut surfaces,
                        &mut max_id,
                    );

                    panes.push(Pane {
                        id: pane.id.into(),
                        cwd: pane.cwd,
                        surfaces,
                        selected_surface,
                    });
                }

                if panes.is_empty() {
                    return Err(SessionError::MissingSelectedPane);
                }

                let selected_pane = if panes
                    .iter()
                    .any(|pane| pane.id == workspace.selected_pane.into())
                {
                    workspace.selected_pane.into()
                } else {
                    panes[0].id
                };
                let layout = reconcile_layout(workspace.layout, &panes);
                let (unread, latest_notification) = workspace_unread_state(&panes);

                workspaces.push(Workspace {
                    id: workspace.id.into(),
                    title: workspace.title,
                    cwd: workspace.cwd,
                    panes,
                    selected_pane,
                    layout,
                    unread,
                    latest_notification,
                });
            }

            if workspaces.is_empty() {
                return Err(SessionError::MissingSelectedWorkspace);
            }

            let selected_workspace = if workspaces
                .iter()
                .any(|workspace| workspace.id == window.selected_workspace.into())
            {
                window.selected_workspace.into()
            } else {
                workspaces[0].id
            };

            windows.push(AppWindow {
                id: window.id.into(),
                workspaces,
                selected_workspace,
            });
        }

        let selected_window = if windows.iter().any(|window| window.id == selected_window_id) {
            selected_window_id
        } else {
            windows[0].id
        };
        let latest_unread_target = newest_unread_target(&windows);
        let next_unread_sequence = 1
            .max(self.next_unread_sequence)
            .max(max_unread_sequence.saturating_add(1));

        Ok(AppModel {
            ids: IdGen::from_next_id(max_id),
            windows,
            selected_window,
            latest_unread_target,
            next_unread_sequence,
        })
    }

    fn max_restored_id(&self) -> u64 {
        let mut max_id = self.selected_window.0;
        if let Some(target) = &self.latest_unread_target {
            max_id = max_id
                .max(target.workspace_id.0)
                .max(target.pane_id.0)
                .max(target.surface_id.0);
        }

        for window in &self.windows {
            max_id = max_id.max(window.id.0).max(window.selected_workspace.0);
            for workspace in &window.workspaces {
                max_id = max_id
                    .max(workspace.id.0)
                    .max(workspace.selected_pane.0)
                    .max(workspace.layout.max_id());
                for pane in &workspace.panes {
                    max_id = max_id.max(pane.id.0).max(pane.selected_surface.0);
                    for surface in &pane.surfaces {
                        max_id = max_id.max(surface.id.0);
                    }
                }
            }
        }

        max_id
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

impl From<SnapshotWindowId> for CoreWindowId {
    fn from(id: SnapshotWindowId) -> Self {
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

impl From<SnapshotWorkspaceId> for CoreWorkspaceId {
    fn from(id: SnapshotWorkspaceId) -> Self {
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

impl From<SnapshotPaneId> for CorePaneId {
    fn from(id: SnapshotPaneId) -> Self {
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

impl From<SnapshotSurfaceId> for CoreSurfaceId {
    fn from(id: SnapshotSurfaceId) -> Self {
        Self(id.0)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SnapshotUnreadTarget {
    pub workspace_id: SnapshotWorkspaceId,
    pub pane_id: SnapshotPaneId,
    pub surface_id: SnapshotSurfaceId,
    pub message: String,
    pub sequence: u64,
}

impl From<&CoreUnreadTarget> for SnapshotUnreadTarget {
    fn from(target: &CoreUnreadTarget) -> Self {
        Self {
            workspace_id: target.workspace_id.into(),
            pane_id: target.pane_id.into(),
            surface_id: target.surface_id.into(),
            message: target.message.clone(),
            sequence: target.sequence,
        }
    }
}

impl From<SnapshotUnreadTarget> for CoreUnreadTarget {
    fn from(target: SnapshotUnreadTarget) -> Self {
        Self {
            workspace_id: target.workspace_id.into(),
            pane_id: target.pane_id.into(),
            surface_id: target.surface_id.into(),
            message: target.message,
            sequence: target.sequence,
        }
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

impl From<SnapshotSurfaceKind> for CoreSurfaceKind {
    fn from(kind: SnapshotSurfaceKind) -> Self {
        match kind {
            SnapshotSurfaceKind::Terminal => Self::Terminal,
            SnapshotSurfaceKind::Browser => Self::Browser,
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

impl From<SnapshotSplitAxis> for CoreSplitAxis {
    fn from(axis: SnapshotSplitAxis) -> Self {
        match axis {
            SnapshotSplitAxis::Horizontal => Self::Horizontal,
            SnapshotSplitAxis::Vertical => Self::Vertical,
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

impl SnapshotSplitTree {
    fn max_id(&self) -> u64 {
        match self {
            Self::Leaf { pane } => pane.0,
            Self::Split { first, second, .. } => first.0.max(second.0),
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
    #[serde(default)]
    pub unread_message: Option<String>,
    #[serde(default)]
    pub unread_sequence: Option<u64>,
}

impl From<&Surface> for SurfaceSnapshot {
    fn from(surface: &Surface) -> Self {
        Self {
            id: surface.id.into(),
            kind: surface.kind.into(),
            title: surface.title.clone(),
            unread: surface.unread,
            unread_message: surface.unread_message.clone(),
            unread_sequence: surface.unread_sequence,
        }
    }
}

impl From<SurfaceSnapshot> for Surface {
    fn from(surface: SurfaceSnapshot) -> Self {
        Self {
            id: surface.id.into(),
            kind: surface.kind.into(),
            title: surface.title,
            unread: surface.unread,
            unread_message: surface.unread_message,
            unread_sequence: surface.unread_sequence,
        }
    }
}

fn reconcile_selected_surface(
    selected_surface: CoreSurfaceId,
    surfaces: &mut Vec<Surface>,
    max_id: &mut u64,
) -> CoreSurfaceId {
    if surfaces
        .iter()
        .any(|surface| surface.id == selected_surface && surface.kind == CoreSurfaceKind::Terminal)
    {
        return selected_surface;
    }

    if let Some(terminal) = surfaces
        .iter()
        .find(|surface| surface.kind == CoreSurfaceKind::Terminal)
    {
        return terminal.id;
    }

    *max_id = max_id.saturating_add(1);
    let replacement = CoreSurfaceId(*max_id);
    surfaces.push(Surface {
        id: replacement,
        kind: CoreSurfaceKind::Terminal,
        title: "Terminal".to_string(),
        unread: false,
        unread_message: None,
        unread_sequence: None,
    });
    replacement
}

fn reconcile_layout(layout: SnapshotSplitTree, panes: &[Pane]) -> CoreSplitTree {
    let first_valid = panes[0].id;
    match layout {
        SnapshotSplitTree::Leaf { pane } => {
            let pane = repair_pane_id(pane.into(), panes, first_valid);
            CoreSplitTree::Leaf(pane)
        }
        SnapshotSplitTree::Split {
            axis,
            first,
            second,
        } => {
            let first_snapshot = first.into();
            let second_snapshot = second.into();
            let first_existing = existing_pane_id(first_snapshot, panes);
            let second_existing = existing_pane_id(second_snapshot, panes);
            let mut first = first_existing.unwrap_or(first_valid);
            let mut second = second_existing.unwrap_or(first_valid);

            if first == second {
                if second_existing.is_none() {
                    if let Some(alternate) = distinct_pane_id(panes, first) {
                        second = alternate;
                    }
                } else if first_existing.is_none() {
                    if let Some(alternate) = distinct_pane_id(panes, second) {
                        first = alternate;
                    }
                } else if let Some(alternate) = distinct_pane_id(panes, first) {
                    second = alternate;
                }
            }

            if first != second {
                CoreSplitTree::Split {
                    axis: axis.into(),
                    first,
                    second,
                }
            } else {
                CoreSplitTree::Leaf(first_valid)
            }
        }
    }
}

fn repair_pane_id(id: CorePaneId, panes: &[Pane], fallback: CorePaneId) -> CorePaneId {
    if panes.iter().any(|pane| pane.id == id) {
        id
    } else {
        fallback
    }
}

fn existing_pane_id(id: CorePaneId, panes: &[Pane]) -> Option<CorePaneId> {
    panes.iter().any(|pane| pane.id == id).then_some(id)
}

fn distinct_pane_id(panes: &[Pane], id: CorePaneId) -> Option<CorePaneId> {
    panes.iter().map(|pane| pane.id).find(|pane| *pane != id)
}

fn workspace_unread_state(panes: &[Pane]) -> (bool, Option<String>) {
    let newest_unread = panes
        .iter()
        .flat_map(|pane| pane.surfaces.iter())
        .filter(|surface| surface.unread)
        .max_by_key(|surface| surface.unread_sequence.unwrap_or_default());

    (
        newest_unread.is_some(),
        newest_unread.and_then(|surface| surface.unread_message.clone()),
    )
}

fn newest_unread_target(windows: &[AppWindow]) -> Option<CoreUnreadTarget> {
    windows
        .iter()
        .flat_map(|window| window.workspaces.iter())
        .flat_map(|workspace| {
            workspace.panes.iter().flat_map(move |pane| {
                pane.surfaces.iter().filter_map(move |surface| {
                    surface.unread.then_some(CoreUnreadTarget {
                        workspace_id: workspace.id,
                        pane_id: pane.id,
                        surface_id: surface.id,
                        message: surface.unread_message.clone().unwrap_or_default(),
                        sequence: surface.unread_sequence.unwrap_or_default(),
                    })
                })
            })
        })
        .max_by_key(|target| target.sequence)
}

#[cfg(test)]
mod tests {
    use super::{AppSnapshot, SessionError};
    use serde_json::Value;
    use umux_core::{
        AppModel, SplitAxis, SplitTree, SurfaceKind,
        ids::{PaneId, SurfaceId, WindowId, WorkspaceId},
    };

    #[test]
    fn snapshot_round_trips_app_owned_state() {
        let mut app = AppModel::new("C:/work/alpha");
        app.split_selected_pane(SplitAxis::Vertical).unwrap();
        app.open_browser_surface("https://example.com".to_string())
            .unwrap();

        let snapshot = AppSnapshot::from_model(&app);
        let json = snapshot.to_json_string().unwrap();
        let snapshot = AppSnapshot::from_json_str(&json).unwrap();

        assert_eq!(snapshot.schema_version, 2);
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
        let json = r#"{"schema_version":3,"selected_window":1,"windows":[]}"#;

        let result = AppSnapshot::from_json_str(json);

        assert!(matches!(
            result,
            Err(SessionError::UnsupportedSchemaVersion(3))
        ));
    }

    #[test]
    fn unsupported_schema_version_is_rejected_before_body_deserialization() {
        let json = r#"{"schema_version":3,"future_shape":{"anything":true}}"#;

        let result = AppSnapshot::from_json_str(json);

        assert!(matches!(
            result,
            Err(SessionError::UnsupportedSchemaVersion(3))
        ));
    }

    #[test]
    fn snapshot_restores_model_and_advances_id_generator() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 1,
  "latest_unread_target": null,
  "next_unread_sequence": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 999,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 999,
          "layout": { "type": "leaf", "pane": 999 },
          "unread": false,
          "latest_notification": null,
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 999,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let mut model = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();

        assert_eq!(model.selected_window, WindowId(1));
        let workspace = model.selected_workspace().unwrap();
        assert_eq!(workspace.id, WorkspaceId(2));
        assert_eq!(workspace.selected_pane, PaneId(3));
        let pane = workspace.selected_pane().unwrap();
        assert_eq!(pane.selected_surface, SurfaceId(4));
        assert_eq!(model.ids.next_surface(), SurfaceId(1000));
    }

    #[test]
    fn restore_repairs_missing_selected_window_to_first_window() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 999,
  "latest_unread_target": null,
  "next_unread_sequence": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 3,
          "layout": { "type": "leaf", "pane": 3 },
          "unread": false,
          "latest_notification": null,
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let model = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();

        assert_eq!(model.selected_window, WindowId(1));
    }

    #[test]
    fn restore_collapses_malformed_split_layout_to_leaf() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 1,
  "latest_unread_target": null,
  "next_unread_sequence": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 3,
          "layout": { "type": "split", "axis": "vertical", "first": 3, "second": 999 },
          "unread": false,
          "latest_notification": null,
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let model = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();
        let workspace = model.selected_workspace().unwrap();

        assert_eq!(workspace.layout, SplitTree::Leaf(PaneId(3)));
    }

    #[test]
    fn restore_repairs_one_missing_split_pane_to_distinct_existing_pane() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 1,
  "latest_unread_target": null,
  "next_unread_sequence": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 5,
          "layout": { "type": "split", "axis": "vertical", "first": 5, "second": 999 },
          "unread": false,
          "latest_notification": null,
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            },
            {
              "id": 5,
              "cwd": "C:/work/alpha",
              "selected_surface": 6,
              "surfaces": [
                {
                  "id": 6,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let model = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();
        let workspace = model.selected_workspace().unwrap();

        assert_eq!(
            workspace.layout,
            SplitTree::Split {
                axis: SplitAxis::Vertical,
                first: PaneId(5),
                second: PaneId(3),
            }
        );
    }

    #[test]
    fn snapshot_preserves_latest_unread_target_and_surface_unread_metadata() {
        let mut app = AppModel::new("C:/work/alpha");
        let surface_id = app.selected_pane().unwrap().selected_surface;
        app.mark_surface_unread(surface_id, "Build finished".to_string())
            .unwrap();

        let restored = AppSnapshot::from_model(&app)
            .to_json_string()
            .and_then(|json| AppSnapshot::from_json_str(&json))
            .and_then(AppSnapshot::into_model)
            .unwrap();

        let target = restored.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.surface_id, surface_id);
        assert_eq!(target.message, "Build finished");
        assert_eq!(target.sequence, 1);
        assert_eq!(restored.next_unread_sequence, 2);
        let surface = restored
            .selected_pane()
            .unwrap()
            .surface(surface_id)
            .unwrap();
        assert_eq!(surface.unread_message, Some("Build finished".to_string()));
        assert_eq!(surface.unread_sequence, Some(1));
    }

    #[test]
    fn restore_recomputes_latest_unread_target_from_surface_metadata() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 1,
  "latest_unread_target": {
    "workspace_id": 2,
    "pane_id": 3,
    "surface_id": 4,
    "message": "Older",
    "sequence": 1
  },
  "next_unread_sequence": 2,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 3,
          "layout": { "type": "leaf", "pane": 3 },
          "unread": true,
          "latest_notification": "Older",
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": true,
                  "unread_message": "Older",
                  "unread_sequence": 1
                },
                {
                  "id": 5,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": true,
                  "unread_message": "Newer",
                  "unread_sequence": 3
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let restored = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();
        let target = restored.latest_unread_target.as_ref().unwrap();

        assert_eq!(target.surface_id, SurfaceId(5));
        assert_eq!(target.message, "Newer");
        assert_eq!(target.sequence, 3);
        assert_eq!(restored.next_unread_sequence, 4);
        let workspace = restored.selected_workspace().unwrap();
        assert_eq!(workspace.latest_notification, Some("Newer".to_string()));
    }

    #[test]
    fn restore_replaces_selected_browser_surface_with_terminal_surface() {
        let mut app = AppModel::new("C:/work/alpha");
        let browser = app
            .open_browser_surface("https://example.com".to_string())
            .unwrap();
        let restored = AppSnapshot::from_model(&app).into_model().unwrap();

        let pane = restored.selected_pane().unwrap();
        assert_ne!(pane.selected_surface, browser);
        assert_eq!(
            pane.surface(pane.selected_surface).unwrap().kind,
            SurfaceKind::Terminal
        );
    }

    #[test]
    fn snapshot_round_trip_preserves_multiple_unread_surface_ordering() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().selected_surface;
        let second = app.open_terminal_surface().unwrap();
        app.mark_surface_unread(first, "First".to_string()).unwrap();
        app.mark_surface_unread(second, "Second".to_string())
            .unwrap();

        let mut restored = AppSnapshot::from_model(&app).into_model().unwrap();

        let target = restored.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.surface_id, second);
        assert_eq!(target.sequence, 2);
        restored.mark_surface_read(second).unwrap();
        let fallback = restored.latest_unread_target.as_ref().unwrap();
        assert_eq!(fallback.surface_id, first);
        assert_eq!(fallback.sequence, 1);
    }

    #[test]
    fn restore_recomputes_workspace_unread_when_snapshot_state_is_stale() {
        let json = r#"{
  "schema_version": 2,
  "selected_window": 1,
  "latest_unread_target": null,
  "next_unread_sequence": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 3,
          "layout": { "type": "leaf", "pane": 3 },
          "unread": true,
          "latest_notification": "Stale",
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                {
                  "id": 4,
                  "kind": "terminal",
                  "title": "Terminal",
                  "unread": false,
                  "unread_message": null,
                  "unread_sequence": null
                }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let restored = AppSnapshot::from_json_str(json)
            .unwrap()
            .into_model()
            .unwrap();
        let workspace = restored.selected_workspace().unwrap();

        assert!(!workspace.unread);
        assert_eq!(workspace.latest_notification, None);
        assert_eq!(restored.latest_unread_target, None);
    }

    #[test]
    fn v1_snapshot_migrates_to_v2_defaults() {
        let json = r#"{
  "schema_version": 1,
  "selected_window": 1,
  "windows": [
    {
      "id": 1,
      "selected_workspace": 2,
      "workspaces": [
        {
          "id": 2,
          "title": "alpha",
          "cwd": "C:/work/alpha",
          "selected_pane": 3,
          "layout": { "type": "leaf", "pane": 3 },
          "unread": false,
          "latest_notification": null,
          "panes": [
            {
              "id": 3,
              "cwd": "C:/work/alpha",
              "selected_surface": 4,
              "surfaces": [
                { "id": 4, "kind": "terminal", "title": "Terminal", "unread": false }
              ]
            }
          ]
        }
      ]
    }
  ]
}"#;

        let snapshot = AppSnapshot::from_json_str(json).unwrap();

        assert_eq!(snapshot.schema_version, 2);
        assert_eq!(snapshot.latest_unread_target, None);
        assert_eq!(snapshot.next_unread_sequence, 1);
        assert_eq!(
            snapshot.windows[0].workspaces[0].panes[0].surfaces[0].unread_message,
            None
        );
        assert_eq!(
            snapshot.windows[0].workspaces[0].panes[0].surfaces[0].unread_sequence,
            None
        );
    }

    #[test]
    fn restore_reports_missing_required_collections() {
        let missing_window = r#"{"schema_version":2,"selected_window":1,"latest_unread_target":null,"next_unread_sequence":1,"windows":[]}"#;
        assert!(matches!(
            AppSnapshot::from_json_str(missing_window)
                .unwrap()
                .into_model(),
            Err(SessionError::MissingSelectedWindow)
        ));

        let missing_workspace = r#"{"schema_version":2,"selected_window":1,"latest_unread_target":null,"next_unread_sequence":1,"windows":[{"id":1,"selected_workspace":2,"workspaces":[]}]}"#;
        assert!(matches!(
            AppSnapshot::from_json_str(missing_workspace)
                .unwrap()
                .into_model(),
            Err(SessionError::MissingSelectedWorkspace)
        ));

        let missing_pane = r#"{"schema_version":2,"selected_window":1,"latest_unread_target":null,"next_unread_sequence":1,"windows":[{"id":1,"selected_workspace":2,"workspaces":[{"id":2,"title":"alpha","cwd":"C:/work/alpha","selected_pane":3,"layout":{"type":"leaf","pane":3},"unread":false,"latest_notification":null,"panes":[]}]}]}"#;
        assert!(matches!(
            AppSnapshot::from_json_str(missing_pane)
                .unwrap()
                .into_model(),
            Err(SessionError::MissingSelectedPane)
        ));

        let missing_surface = r#"{"schema_version":2,"selected_window":1,"latest_unread_target":null,"next_unread_sequence":1,"windows":[{"id":1,"selected_workspace":2,"workspaces":[{"id":2,"title":"alpha","cwd":"C:/work/alpha","selected_pane":3,"layout":{"type":"leaf","pane":3},"unread":false,"latest_notification":null,"panes":[{"id":3,"cwd":"C:/work/alpha","selected_surface":4,"surfaces":[]}]}]}]}"#;
        assert!(matches!(
            AppSnapshot::from_json_str(missing_surface)
                .unwrap()
                .into_model(),
            Err(SessionError::MissingSelectedSurface)
        ));
    }

    #[test]
    fn snapshot_json_uses_stable_v2_wire_contract() {
        let mut app = AppModel::new("C:/work/alpha");
        app.split_selected_pane(SplitAxis::Vertical).unwrap();
        app.open_browser_surface("https://example.com".to_string())
            .unwrap();
        let selected_surface = app.selected_pane().unwrap().selected_surface;
        app.mark_surface_unread(selected_surface, "Browser updated".to_string())
            .unwrap();

        let json = AppSnapshot::from_model(&app).to_json_string().unwrap();
        let value: Value = serde_json::from_str(&json).unwrap();

        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["selected_window"], 1);
        assert_eq!(value["latest_unread_target"]["workspace_id"], 2);
        assert_eq!(value["latest_unread_target"]["pane_id"], 5);
        assert_eq!(value["latest_unread_target"]["surface_id"], 7);
        assert_eq!(value["latest_unread_target"]["message"], "Browser updated");
        assert_eq!(value["latest_unread_target"]["sequence"], 1);
        assert_eq!(value["next_unread_sequence"], 2);

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
        assert_eq!(browser_surface["unread_message"], "Browser updated");
        assert_eq!(browser_surface["unread_sequence"], 1);
    }
}

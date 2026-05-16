// SPDX-License-Identifier: GPL-3.0-or-later

use umux_core::{PaneId, SplitAxis, SurfaceId, WorkspaceId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AppAction {
    NewWorkspace {
        cwd: String,
        title: Option<String>,
    },
    SelectWorkspace(WorkspaceId),
    RenameWorkspace {
        workspace_id: WorkspaceId,
        title: String,
    },
    CloseWorkspace(WorkspaceId),
    SplitPane(SplitAxis),
    ClosePane(PaneId),
    SelectPane(PaneId),
    NewTerminalTab,
    SelectSurface(SurfaceId),
    CloseSurface(SurfaceId),
    JumpLatestUnread,
    MarkSurfaceRead(SurfaceId),
    SaveSessionNow,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AppActionOutcome {
    pub selected_workspace_changed: bool,
    pub selected_pane_changed: bool,
    pub selected_surface_changed: bool,
    pub spawned_surfaces: Vec<SurfaceId>,
    pub closed_surfaces: Vec<SurfaceId>,
    pub should_save_session: bool,
}

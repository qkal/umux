// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ids::{IdGen, PaneId, SurfaceId, WindowId, WorkspaceId};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SurfaceKind {
    Terminal,
    Browser,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum SplitTree {
    Leaf(PaneId),
    Split {
        axis: SplitAxis,
        first: Box<SplitTree>,
        second: Box<SplitTree>,
    },
}

impl SplitTree {
    fn split_leaf(&mut self, target: PaneId, axis: SplitAxis, new_pane: PaneId) -> bool {
        match self {
            SplitTree::Leaf(pane_id) if *pane_id == target => {
                *self = SplitTree::Split {
                    axis,
                    first: Box::new(SplitTree::Leaf(target)),
                    second: Box::new(SplitTree::Leaf(new_pane)),
                };
                true
            }
            SplitTree::Leaf(_) => false,
            SplitTree::Split { first, second, .. } => {
                first.split_leaf(target, axis, new_pane)
                    || second.split_leaf(target, axis, new_pane)
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Surface {
    pub id: SurfaceId,
    pub kind: SurfaceKind,
    pub title: String,
    pub unread: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pane {
    pub id: PaneId,
    pub cwd: PathBuf,
    pub surfaces: Vec<Surface>,
    pub selected_surface: SurfaceId,
}

impl Pane {
    pub fn surface(&self, id: SurfaceId) -> Option<&Surface> {
        self.surfaces.iter().find(|surface| surface.id == id)
    }

    pub fn surface_mut(&mut self, id: SurfaceId) -> Option<&mut Surface> {
        self.surfaces.iter_mut().find(|surface| surface.id == id)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Workspace {
    pub id: WorkspaceId,
    pub title: String,
    pub cwd: PathBuf,
    pub panes: Vec<Pane>,
    pub selected_pane: PaneId,
    pub layout: SplitTree,
    pub unread: bool,
    pub latest_notification: Option<String>,
}

impl Workspace {
    pub fn pane(&self, id: PaneId) -> Option<&Pane> {
        self.panes.iter().find(|pane| pane.id == id)
    }

    pub fn pane_mut(&mut self, id: PaneId) -> Option<&mut Pane> {
        self.panes.iter_mut().find(|pane| pane.id == id)
    }

    pub fn selected_pane(&self) -> Option<&Pane> {
        self.pane(self.selected_pane)
    }

    pub fn selected_pane_mut(&mut self) -> Option<&mut Pane> {
        self.pane_mut(self.selected_pane)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppWindow {
    pub id: WindowId,
    pub workspaces: Vec<Workspace>,
    pub selected_workspace: WorkspaceId,
}

impl AppWindow {
    pub fn workspace(&self, id: WorkspaceId) -> Option<&Workspace> {
        self.workspaces.iter().find(|workspace| workspace.id == id)
    }

    pub fn workspace_mut(&mut self, id: WorkspaceId) -> Option<&mut Workspace> {
        self.workspaces
            .iter_mut()
            .find(|workspace| workspace.id == id)
    }

    pub fn selected_workspace(&self) -> Option<&Workspace> {
        self.workspace(self.selected_workspace)
    }

    pub fn selected_workspace_mut(&mut self) -> Option<&mut Workspace> {
        self.workspace_mut(self.selected_workspace)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AppModel {
    #[serde(skip)]
    pub ids: IdGen,
    pub windows: Vec<AppWindow>,
    pub selected_window: WindowId,
}

impl AppModel {
    pub fn new(cwd: PathBuf) -> Self {
        let mut ids = IdGen::new();
        let window_id = ids.window_id();
        let workspace_id = ids.workspace_id();
        let pane_id = ids.pane_id();
        let surface_id = ids.surface_id();
        let title = workspace_title(&cwd);

        let surface = terminal_surface(surface_id);
        let pane = Pane {
            id: pane_id,
            cwd: cwd.clone(),
            surfaces: vec![surface],
            selected_surface: surface_id,
        };
        let workspace = Workspace {
            id: workspace_id,
            title,
            cwd,
            panes: vec![pane],
            selected_pane: pane_id,
            layout: SplitTree::Leaf(pane_id),
            unread: false,
            latest_notification: None,
        };
        let window = AppWindow {
            id: window_id,
            workspaces: vec![workspace],
            selected_workspace: workspace_id,
        };

        Self {
            ids,
            windows: vec![window],
            selected_window: window_id,
        }
    }

    pub fn selected_window(&self) -> Result<&AppWindow, ModelError> {
        self.windows
            .iter()
            .find(|window| window.id == self.selected_window)
            .ok_or(ModelError::NoSelectedWindow)
    }

    pub fn selected_window_mut(&mut self) -> Result<&mut AppWindow, ModelError> {
        self.windows
            .iter_mut()
            .find(|window| window.id == self.selected_window)
            .ok_or(ModelError::NoSelectedWindow)
    }

    pub fn selected_workspace(&self) -> Result<&Workspace, ModelError> {
        self.selected_window()?
            .selected_workspace()
            .ok_or(ModelError::NoSelectedWorkspace)
    }

    pub fn selected_workspace_mut(&mut self) -> Result<&mut Workspace, ModelError> {
        self.selected_window_mut()?
            .selected_workspace_mut()
            .ok_or(ModelError::NoSelectedWorkspace)
    }

    pub fn selected_pane(&self) -> Result<&Pane, ModelError> {
        self.selected_workspace()?
            .selected_pane()
            .ok_or(ModelError::NoSelectedPane)
    }

    pub fn selected_pane_mut(&mut self) -> Result<&mut Pane, ModelError> {
        self.selected_workspace_mut()?
            .selected_pane_mut()
            .ok_or(ModelError::NoSelectedPane)
    }

    pub fn split_selected_pane(&mut self, axis: SplitAxis) -> Result<PaneId, ModelError> {
        let selected_pane_id = self.selected_workspace()?.selected_pane;
        let cwd = self
            .selected_workspace()?
            .pane(selected_pane_id)
            .ok_or(ModelError::UnknownPane(selected_pane_id))?
            .cwd
            .clone();
        let new_pane_id = self.ids.pane_id();
        let surface_id = self.ids.surface_id();
        let new_pane = Pane {
            id: new_pane_id,
            cwd,
            surfaces: vec![terminal_surface(surface_id)],
            selected_surface: surface_id,
        };

        let workspace = self.selected_workspace_mut()?;
        workspace.panes.push(new_pane);
        workspace
            .layout
            .split_leaf(selected_pane_id, axis, new_pane_id);
        workspace.selected_pane = new_pane_id;

        Ok(new_pane_id)
    }

    pub fn open_browser_surface(&mut self, url: String) -> Result<SurfaceId, ModelError> {
        let surface_id = self.ids.surface_id();
        let surface = Surface {
            id: surface_id,
            kind: SurfaceKind::Browser,
            title: url,
            unread: false,
        };

        let pane = self.selected_pane_mut()?;
        pane.surfaces.push(surface);
        pane.selected_surface = surface_id;

        Ok(surface_id)
    }

    pub fn mark_surface_unread(
        &mut self,
        surface_id: SurfaceId,
        message: String,
    ) -> Result<(), ModelError> {
        let workspace = self.selected_workspace_mut()?;

        let surface = workspace
            .panes
            .iter_mut()
            .find_map(|pane| pane.surface_mut(surface_id))
            .ok_or(ModelError::UnknownSurface(surface_id))?;

        surface.unread = true;
        workspace.unread = true;
        workspace.latest_notification = Some(message);

        Ok(())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ModelError {
    #[error("no selected window")]
    NoSelectedWindow,
    #[error("no selected workspace")]
    NoSelectedWorkspace,
    #[error("no selected pane")]
    NoSelectedPane,
    #[error("unknown pane {0:?}")]
    UnknownPane(PaneId),
    #[error("unknown surface {0:?}")]
    UnknownSurface(SurfaceId),
}

fn workspace_title(cwd: &std::path::Path) -> String {
    cwd.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Workspace")
        .to_string()
}

fn terminal_surface(id: SurfaceId) -> Surface {
    Surface {
        id,
        kind: SurfaceKind::Terminal,
        title: "Terminal".to_string(),
        unread: false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{AppModel, SplitAxis, SurfaceKind};

    #[test]
    fn new_app_has_one_workspace_one_pane_and_one_terminal_surface() {
        let cwd = PathBuf::from("C:/Users/Better/projects/alpha");
        let app = AppModel::new(cwd.clone());

        assert_eq!(app.windows.len(), 1);
        assert_eq!(app.selected_window, app.windows[0].id);

        let window = app.selected_window().unwrap();
        assert_eq!(window.workspaces.len(), 1);
        assert_eq!(window.selected_workspace, window.workspaces[0].id);

        let workspace = window.selected_workspace().unwrap();
        assert_eq!(workspace.title, "alpha");
        assert_eq!(workspace.cwd, cwd);
        assert_eq!(workspace.panes.len(), 1);
        assert_eq!(workspace.selected_pane, workspace.panes[0].id);
        assert!(!workspace.unread);
        assert_eq!(workspace.latest_notification, None);

        let pane = workspace.selected_pane().unwrap();
        assert_eq!(pane.cwd, cwd);
        assert_eq!(pane.surfaces.len(), 1);
        assert_eq!(pane.selected_surface, pane.surfaces[0].id);

        let surface = pane.surface(pane.selected_surface).unwrap();
        assert_eq!(surface.kind, SurfaceKind::Terminal);
        assert_eq!(surface.title, "Terminal");
        assert!(!surface.unread);
    }

    #[test]
    fn split_selected_pane_creates_second_pane_with_inherited_cwd() {
        let cwd = PathBuf::from("C:/work/project");
        let mut app = AppModel::new(cwd.clone());

        let new_pane_id = app.split_selected_pane(SplitAxis::Vertical).unwrap();

        let workspace = app.selected_workspace().unwrap();
        assert_eq!(workspace.panes.len(), 2);
        assert_eq!(workspace.selected_pane, new_pane_id);

        let new_pane = workspace.pane(new_pane_id).unwrap();
        assert_eq!(new_pane.cwd, cwd);
        assert_eq!(new_pane.surfaces.len(), 1);
        assert_eq!(
            new_pane.surface(new_pane.selected_surface).unwrap().kind,
            SurfaceKind::Terminal
        );
    }

    #[test]
    fn browser_surface_is_created_in_selected_pane() {
        let mut app = AppModel::new(PathBuf::from("C:/work/project"));

        let surface_id = app
            .open_browser_surface("https://example.com".to_string())
            .unwrap();

        let pane = app.selected_pane().unwrap();
        assert_eq!(pane.surfaces.len(), 2);
        assert_eq!(pane.selected_surface, surface_id);

        let surface = pane.surface(surface_id).unwrap();
        assert_eq!(surface.kind, SurfaceKind::Browser);
        assert_eq!(surface.title, "https://example.com");
        assert!(!surface.unread);
    }

    #[test]
    fn unread_state_rolls_up_from_surface_to_workspace() {
        let mut app = AppModel::new(PathBuf::from("C:/work/project"));
        let surface_id = app.selected_pane().unwrap().selected_surface;

        app.mark_surface_unread(surface_id, "Build finished".to_string())
            .unwrap();

        let workspace = app.selected_workspace().unwrap();
        let surface = workspace
            .selected_pane()
            .unwrap()
            .surface(surface_id)
            .unwrap();

        assert!(surface.unread);
        assert!(workspace.unread);
        assert_eq!(
            workspace.latest_notification,
            Some("Build finished".to_string())
        );
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

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
        first: PaneId,
        second: PaneId,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Surface {
    pub id: SurfaceId,
    pub kind: SurfaceKind,
    pub title: String,
    pub unread: bool,
    #[serde(default)]
    pub unread_message: Option<String>,
    #[serde(default)]
    pub unread_sequence: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct UnreadTarget {
    pub workspace_id: WorkspaceId,
    pub pane_id: PaneId,
    pub surface_id: SurfaceId,
    pub message: String,
    pub sequence: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Pane {
    pub id: PaneId,
    pub cwd: String,
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
    pub cwd: String,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AppModel {
    pub ids: IdGen,
    pub windows: Vec<AppWindow>,
    pub selected_window: WindowId,
    pub latest_unread_target: Option<UnreadTarget>,
    pub next_unread_sequence: u64,
}

impl AppModel {
    pub fn new(cwd: impl Into<String>) -> Self {
        let cwd = cwd.into();
        let mut ids = IdGen::new();
        let window_id = ids.next_window();
        let workspace_id = ids.next_workspace();
        let pane_id = ids.next_pane();
        let surface_id = ids.next_surface();
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
            latest_unread_target: None,
            next_unread_sequence: 1,
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

    pub fn create_workspace(
        &mut self,
        cwd: impl Into<String>,
        title: Option<String>,
    ) -> Result<WorkspaceId, ModelError> {
        let cwd = cwd.into();
        let workspace_id = self.ids.next_workspace();
        let pane_id = self.ids.next_pane();
        let surface_id = self.ids.next_surface();
        let workspace = Workspace {
            id: workspace_id,
            title: title.unwrap_or_else(|| workspace_title(&cwd)),
            cwd: cwd.clone(),
            panes: vec![Pane {
                id: pane_id,
                cwd,
                surfaces: vec![terminal_surface(surface_id)],
                selected_surface: surface_id,
            }],
            selected_pane: pane_id,
            layout: SplitTree::Leaf(pane_id),
            unread: false,
            latest_notification: None,
        };

        let window = self.selected_window_mut()?;
        window.workspaces.push(workspace);
        window.selected_workspace = workspace_id;
        Ok(workspace_id)
    }

    pub fn select_workspace(&mut self, workspace_id: WorkspaceId) -> Result<(), ModelError> {
        let window = self.selected_window_mut()?;
        if window.workspace(workspace_id).is_none() {
            return Err(ModelError::UnknownWorkspace(workspace_id));
        }
        window.selected_workspace = workspace_id;
        Ok(())
    }

    pub fn rename_workspace(
        &mut self,
        workspace_id: WorkspaceId,
        title: String,
    ) -> Result<(), ModelError> {
        let workspace = self
            .selected_window_mut()?
            .workspace_mut(workspace_id)
            .ok_or(ModelError::UnknownWorkspace(workspace_id))?;
        workspace.title = title;
        Ok(())
    }

    pub fn close_workspace(
        &mut self,
        workspace_id: WorkspaceId,
    ) -> Result<Vec<SurfaceId>, ModelError> {
        let window = self.selected_window_mut()?;
        if window.workspaces.len() == 1 {
            return Err(ModelError::CannotCloseLastWorkspace);
        }
        let index = window
            .workspaces
            .iter()
            .position(|workspace| workspace.id == workspace_id)
            .ok_or(ModelError::UnknownWorkspace(workspace_id))?;
        let removed = window.workspaces.remove(index);
        let removed_surfaces = workspace_surface_ids(&removed);
        if window.selected_workspace == workspace_id {
            let next_index = index.saturating_sub(1).min(window.workspaces.len() - 1);
            window.selected_workspace = window.workspaces[next_index].id;
        }
        self.clear_unread_target_if_removed(&removed_surfaces);
        Ok(removed_surfaces)
    }

    pub fn select_pane(&mut self, pane_id: PaneId) -> Result<(), ModelError> {
        let workspace = self.selected_workspace_mut()?;
        if workspace.pane(pane_id).is_none() {
            return Err(ModelError::UnknownPane(pane_id));
        }
        workspace.selected_pane = pane_id;
        Ok(())
    }

    pub fn close_pane(&mut self, pane_id: PaneId) -> Result<Vec<SurfaceId>, ModelError> {
        let removed_surfaces = {
            let workspace = self.selected_workspace_mut()?;
            if workspace.panes.len() == 1 {
                return Err(ModelError::CannotCloseLastPane);
            }
            let index = workspace
                .panes
                .iter()
                .position(|pane| pane.id == pane_id)
                .ok_or(ModelError::UnknownPane(pane_id))?;
            let removed = workspace.panes.remove(index);
            let removed_surfaces = removed
                .surfaces
                .iter()
                .map(|surface| surface.id)
                .collect::<Vec<_>>();
            let remaining = workspace.panes[0].id;
            workspace.selected_pane = remaining;
            workspace.layout = SplitTree::Leaf(remaining);
            recompute_workspace_unread(workspace);
            removed_surfaces
        };
        self.clear_unread_target_if_removed(&removed_surfaces);
        Ok(removed_surfaces)
    }

    pub fn split_selected_pane(&mut self, axis: SplitAxis) -> Result<PaneId, ModelError> {
        let workspace = self.selected_workspace()?;
        if matches!(workspace.layout, SplitTree::Split { .. }) {
            return Err(ModelError::LayoutAlreadySplit);
        }

        let selected_pane_id = workspace.selected_pane;
        let cwd = workspace
            .pane(selected_pane_id)
            .ok_or(ModelError::UnknownPane(selected_pane_id))?
            .cwd
            .clone();
        let new_pane_id = self.ids.next_pane();
        let surface_id = self.ids.next_surface();
        let new_pane = Pane {
            id: new_pane_id,
            cwd,
            surfaces: vec![terminal_surface(surface_id)],
            selected_surface: surface_id,
        };

        let workspace = self.selected_workspace_mut()?;
        workspace.panes.push(new_pane);
        workspace.layout = SplitTree::Split {
            axis,
            first: selected_pane_id,
            second: new_pane_id,
        };
        workspace.selected_pane = new_pane_id;

        Ok(new_pane_id)
    }

    pub fn open_browser_surface(&mut self, url: String) -> Result<SurfaceId, ModelError> {
        let surface_id = self.ids.next_surface();
        let surface = Surface {
            id: surface_id,
            kind: SurfaceKind::Browser,
            title: url,
            unread: false,
            unread_message: None,
            unread_sequence: None,
        };

        let pane = self.selected_pane_mut()?;
        pane.surfaces.push(surface);
        pane.selected_surface = surface_id;

        Ok(surface_id)
    }

    pub fn open_terminal_surface(&mut self) -> Result<SurfaceId, ModelError> {
        let surface_id = self.ids.next_surface();
        let pane = self.selected_pane_mut()?;
        pane.surfaces.push(terminal_surface(surface_id));
        pane.selected_surface = surface_id;
        Ok(surface_id)
    }

    pub fn select_surface(&mut self, surface_id: SurfaceId) -> Result<(), ModelError> {
        let pane = self.selected_pane_mut()?;
        if pane.surface(surface_id).is_none() {
            return Err(ModelError::UnknownSurface(surface_id));
        }
        pane.selected_surface = surface_id;
        Ok(())
    }

    pub fn close_surface(&mut self, surface_id: SurfaceId) -> Result<Vec<SurfaceId>, ModelError> {
        let mut removed = Vec::new();
        let needs_replacement = {
            let pane = self.selected_pane_mut()?;
            let index = pane
                .surfaces
                .iter()
                .position(|surface| surface.id == surface_id)
                .ok_or(ModelError::UnknownSurface(surface_id))?;
            pane.surfaces.remove(index);
            removed.push(surface_id);
            if pane.surfaces.is_empty() {
                true
            } else {
                let next_index = index.saturating_sub(1).min(pane.surfaces.len() - 1);
                pane.selected_surface = pane.surfaces[next_index].id;
                false
            }
        };

        if needs_replacement {
            let replacement = self.ids.next_surface();
            let pane = self.selected_pane_mut()?;
            pane.surfaces.push(terminal_surface(replacement));
            pane.selected_surface = replacement;
        }

        self.clear_unread_target_if_removed(&removed);
        self.recompute_selected_workspace_unread()?;
        Ok(removed)
    }

    pub fn mark_surface_unread(
        &mut self,
        surface_id: SurfaceId,
        message: String,
    ) -> Result<(), ModelError> {
        let (window_index, workspace_index, pane_index, surface_index) = self
            .surface_location(surface_id)
            .ok_or(ModelError::UnknownSurface(surface_id))?;
        let sequence = self.next_unread_sequence;
        self.next_unread_sequence += 1;

        let workspace = &mut self.windows[window_index].workspaces[workspace_index];
        let workspace_id = workspace.id;
        let pane_id = workspace.panes[pane_index].id;
        let surface = &mut workspace.panes[pane_index].surfaces[surface_index];

        surface.unread = true;
        surface.unread_message = Some(message.clone());
        surface.unread_sequence = Some(sequence);
        workspace.unread = true;
        workspace.latest_notification = Some(message.clone());
        self.latest_unread_target = Some(UnreadTarget {
            workspace_id,
            pane_id,
            surface_id,
            message,
            sequence,
        });

        Ok(())
    }

    pub fn mark_surface_read(&mut self, surface_id: SurfaceId) -> Result<(), ModelError> {
        let (window_index, workspace_index, pane_index, surface_index) = self
            .surface_location(surface_id)
            .ok_or(ModelError::UnknownSurface(surface_id))?;
        let workspace = &mut self.windows[window_index].workspaces[workspace_index];
        let surface = &mut workspace.panes[pane_index].surfaces[surface_index];
        surface.unread = false;
        surface.unread_message = None;
        surface.unread_sequence = None;
        recompute_workspace_unread(workspace);
        self.latest_unread_target = newest_unread_target(self);
        Ok(())
    }

    fn clear_unread_target_if_removed(&mut self, removed_surfaces: &[SurfaceId]) {
        if self
            .latest_unread_target
            .as_ref()
            .is_some_and(|target| removed_surfaces.contains(&target.surface_id))
        {
            self.latest_unread_target = newest_unread_target(self);
        }
    }

    fn recompute_selected_workspace_unread(&mut self) -> Result<(), ModelError> {
        let workspace = self.selected_workspace_mut()?;
        recompute_workspace_unread(workspace);
        Ok(())
    }

    fn surface_location(&self, surface_id: SurfaceId) -> Option<(usize, usize, usize, usize)> {
        self.windows
            .iter()
            .enumerate()
            .find_map(|(window_index, window)| {
                window
                    .workspaces
                    .iter()
                    .enumerate()
                    .find_map(|(workspace_index, workspace)| {
                        workspace
                            .panes
                            .iter()
                            .enumerate()
                            .find_map(|(pane_index, pane)| {
                                pane.surfaces
                                    .iter()
                                    .position(|surface| surface.id == surface_id)
                                    .map(|surface_index| {
                                        (window_index, workspace_index, pane_index, surface_index)
                                    })
                            })
                    })
            })
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
    #[error("layout is already split")]
    LayoutAlreadySplit,
    #[error("unknown workspace {0:?}")]
    UnknownWorkspace(WorkspaceId),
    #[error("unknown pane {0:?}")]
    UnknownPane(PaneId),
    #[error("unknown surface {0:?}")]
    UnknownSurface(SurfaceId),
    #[error("cannot close the last workspace")]
    CannotCloseLastWorkspace,
    #[error("cannot close the last pane")]
    CannotCloseLastPane,
}

fn workspace_title(cwd: &str) -> String {
    cwd.rsplit(['/', '\\'])
        .find(|segment| !segment.is_empty())
        .unwrap_or("Workspace")
        .to_string()
}

fn terminal_surface(id: SurfaceId) -> Surface {
    Surface {
        id,
        kind: SurfaceKind::Terminal,
        title: "Terminal".to_string(),
        unread: false,
        unread_message: None,
        unread_sequence: None,
    }
}

fn workspace_surface_ids(workspace: &Workspace) -> Vec<SurfaceId> {
    workspace
        .panes
        .iter()
        .flat_map(|pane| pane.surfaces.iter().map(|surface| surface.id))
        .collect()
}

fn newest_unread_target(app: &AppModel) -> Option<UnreadTarget> {
    app.windows
        .iter()
        .flat_map(|window| window.workspaces.iter())
        .flat_map(|workspace| {
            workspace.panes.iter().flat_map(move |pane| {
                pane.surfaces.iter().filter_map(move |surface| {
                    surface.unread.then_some(UnreadTarget {
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

fn recompute_workspace_unread(workspace: &mut Workspace) {
    let newest_unread = workspace
        .panes
        .iter()
        .flat_map(|pane| pane.surfaces.iter())
        .filter(|surface| surface.unread)
        .max_by_key(|surface| surface.unread_sequence.unwrap_or_default());
    workspace.unread = newest_unread.is_some();
    workspace.latest_notification =
        newest_unread.and_then(|surface| surface.unread_message.clone());
}

#[cfg(test)]
mod tests {
    use super::{AppModel, ModelError, SplitAxis, SplitTree, SurfaceKind};
    use crate::ids::{PaneId, SurfaceId, WindowId, WorkspaceId};

    #[test]
    fn id_gen_returns_incrementing_ids_starting_at_one() {
        let app = AppModel::new("C:/work/project");

        assert_eq!(app.selected_window, WindowId(1));
        let workspace = app.selected_workspace().unwrap();
        assert_eq!(workspace.id, WorkspaceId(2));
        assert_eq!(workspace.selected_pane, PaneId(3));
        assert_eq!(
            workspace.selected_pane().unwrap().selected_surface,
            SurfaceId(4)
        );
    }

    #[test]
    fn new_app_has_one_workspace_one_pane_and_one_terminal_surface() {
        let cwd = "C:/Users/Better/projects/alpha";
        let app = AppModel::new(cwd);

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
        let cwd = "C:/work/project";
        let mut app = AppModel::new(cwd);
        let first_pane_id = app.selected_pane().unwrap().id;

        let new_pane_id = app.split_selected_pane(SplitAxis::Vertical).unwrap();

        let workspace = app.selected_workspace().unwrap();
        assert_eq!(workspace.panes.len(), 2);
        assert_eq!(workspace.selected_pane, new_pane_id);
        assert_eq!(
            workspace.layout,
            SplitTree::Split {
                axis: SplitAxis::Vertical,
                first: first_pane_id,
                second: new_pane_id,
            }
        );

        let new_pane = workspace.pane(new_pane_id).unwrap();
        assert_eq!(new_pane.cwd, cwd);
        assert_eq!(new_pane.surfaces.len(), 1);
        assert_eq!(
            new_pane.surface(new_pane.selected_surface).unwrap().kind,
            SurfaceKind::Terminal
        );
    }

    #[test]
    fn second_split_is_rejected_without_changing_panes_or_layout() {
        let mut app = AppModel::new("C:/work/project");
        app.split_selected_pane(SplitAxis::Vertical).unwrap();
        let workspace = app.selected_workspace().unwrap();
        let pane_count = workspace.panes.len();
        let selected_pane = workspace.selected_pane;
        let layout = workspace.layout.clone();

        let result = app.split_selected_pane(SplitAxis::Horizontal);

        assert_eq!(result, Err(ModelError::LayoutAlreadySplit));
        let workspace = app.selected_workspace().unwrap();
        assert_eq!(workspace.panes.len(), pane_count);
        assert_eq!(workspace.selected_pane, selected_pane);
        assert_eq!(workspace.layout, layout);
    }

    #[test]
    fn browser_surface_is_created_in_selected_pane() {
        let mut app = AppModel::new("C:/work/project");

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
        let mut app = AppModel::new("C:/work/project");
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

    #[test]
    fn app_can_create_select_rename_and_close_workspaces() {
        let mut app = AppModel::new("C:/work/alpha");
        let original = app.selected_workspace().unwrap().id;

        let beta = app
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();
        assert_eq!(app.selected_workspace().unwrap().id, beta);
        assert_eq!(app.selected_workspace().unwrap().title, "Beta");

        app.rename_workspace(beta, "Beta renamed".to_string())
            .unwrap();
        assert_eq!(app.selected_workspace().unwrap().title, "Beta renamed");

        app.select_workspace(original).unwrap();
        app.close_workspace(beta).unwrap();

        let window = app.selected_window().unwrap();
        assert_eq!(window.workspaces.len(), 1);
        assert_eq!(window.selected_workspace, original);
    }

    #[test]
    fn app_can_open_select_and_close_terminal_tabs_without_emptying_pane() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().selected_surface;

        let second = app.open_terminal_surface().unwrap();
        assert_eq!(app.selected_pane().unwrap().selected_surface, second);
        assert_eq!(app.selected_pane().unwrap().surfaces.len(), 2);

        app.select_surface(first).unwrap();
        app.close_surface(first).unwrap();
        assert_eq!(app.selected_pane().unwrap().selected_surface, second);

        app.close_surface(second).unwrap();
        let pane = app.selected_pane().unwrap();
        assert_eq!(pane.surfaces.len(), 1);
        assert_eq!(
            pane.surface(pane.selected_surface).unwrap().kind,
            SurfaceKind::Terminal
        );
    }

    #[test]
    fn closing_split_pane_returns_workspace_to_leaf_layout() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().id;
        let second = app.split_selected_pane(SplitAxis::Vertical).unwrap();

        app.close_pane(second).unwrap();

        let workspace = app.selected_workspace().unwrap();
        assert_eq!(workspace.panes.len(), 1);
        assert_eq!(workspace.selected_pane, first);
        assert_eq!(workspace.layout, SplitTree::Leaf(first));
    }

    #[test]
    fn unread_target_tracks_workspace_pane_and_surface() {
        let mut app = AppModel::new("C:/work/alpha");
        let workspace_id = app.selected_workspace().unwrap().id;
        let pane_id = app.selected_pane().unwrap().id;
        let surface_id = app.selected_pane().unwrap().selected_surface;

        app.mark_surface_unread(surface_id, "Build finished".to_string())
            .unwrap();

        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.workspace_id, workspace_id);
        assert_eq!(target.pane_id, pane_id);
        assert_eq!(target.surface_id, surface_id);
        assert_eq!(target.message, "Build finished");
        assert_eq!(target.sequence, 1);
        let surface = app.selected_pane().unwrap().surface(surface_id).unwrap();
        assert_eq!(surface.unread_message, Some("Build finished".to_string()));
        assert_eq!(surface.unread_sequence, Some(1));

        app.mark_surface_read(surface_id).unwrap();
        assert_eq!(app.latest_unread_target, None);
        let surface = app.selected_pane().unwrap().surface(surface_id).unwrap();
        assert_eq!(surface.unread_message, None);
        assert_eq!(surface.unread_sequence, None);
        assert!(!app.selected_workspace().unwrap().unread);
    }

    #[test]
    fn unread_marking_finds_surfaces_in_non_selected_workspace() {
        let mut app = AppModel::new("C:/work/alpha");
        let alpha = app.selected_workspace().unwrap().id;
        let beta = app
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();
        let beta_surface = app.selected_pane().unwrap().selected_surface;

        app.select_workspace(alpha).unwrap();
        app.mark_surface_unread(beta_surface, "Beta done".to_string())
            .unwrap();

        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.workspace_id, beta);
        assert_eq!(target.surface_id, beta_surface);
        assert_eq!(target.message, "Beta done");
        assert!(
            app.selected_window()
                .unwrap()
                .workspace(beta)
                .unwrap()
                .unread
        );
        assert!(!app.selected_workspace().unwrap().unread);

        app.mark_surface_read(beta_surface).unwrap();
        assert_eq!(app.latest_unread_target, None);
        assert!(
            !app.selected_window()
                .unwrap()
                .workspace(beta)
                .unwrap()
                .unread
        );
    }

    #[test]
    fn closing_pane_with_only_unread_surface_clears_workspace_unread_state() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().id;
        let second = app.split_selected_pane(SplitAxis::Vertical).unwrap();
        let unread_surface = app.selected_pane().unwrap().selected_surface;

        app.mark_surface_unread(unread_surface, "Done".to_string())
            .unwrap();
        app.select_pane(first).unwrap();
        app.close_pane(second).unwrap();

        let workspace = app.selected_workspace().unwrap();
        assert!(!workspace.unread);
        assert_eq!(workspace.latest_notification, None);
        assert_eq!(app.latest_unread_target, None);
    }

    #[test]
    fn closing_surface_with_latest_unread_target_falls_back_to_remaining_unread_surface() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().selected_surface;
        let second = app.open_terminal_surface().unwrap();

        app.mark_surface_unread(first, "First".to_string()).unwrap();
        app.mark_surface_unread(second, "Second".to_string())
            .unwrap();

        app.close_surface(second).unwrap();

        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.surface_id, first);
        assert_ne!(target.surface_id, second);
        assert_eq!(target.message, "First");
        assert_eq!(target.sequence, 1);
    }

    #[test]
    fn reading_latest_unread_target_falls_back_to_remaining_surface_metadata() {
        let mut app = AppModel::new("C:/work/alpha");
        let first = app.selected_pane().unwrap().selected_surface;
        let second = app.open_terminal_surface().unwrap();

        app.mark_surface_unread(first, "First".to_string()).unwrap();
        app.mark_surface_unread(second, "Second".to_string())
            .unwrap();

        app.mark_surface_read(second).unwrap();

        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.surface_id, first);
        assert_eq!(target.message, "First");
        assert_eq!(target.sequence, 1);
        assert_eq!(
            app.selected_workspace().unwrap().latest_notification,
            Some("First".to_string())
        );
    }

    #[test]
    fn closing_pane_with_latest_notification_preserves_remaining_workspace_message() {
        let mut app = AppModel::new("C:/work/alpha");
        let first_surface = app.selected_pane().unwrap().selected_surface;
        let second_pane = app.split_selected_pane(SplitAxis::Vertical).unwrap();
        let second_surface = app.selected_pane().unwrap().selected_surface;

        app.mark_surface_unread(first_surface, "First".to_string())
            .unwrap();
        app.mark_surface_unread(second_surface, "Second".to_string())
            .unwrap();
        app.close_pane(second_pane).unwrap();

        let workspace = app.selected_workspace().unwrap();
        assert!(workspace.unread);
        assert_eq!(workspace.latest_notification, Some("First".to_string()));
        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.surface_id, first_surface);
        assert_eq!(target.message, "First");
        assert_eq!(target.sequence, 1);
    }

    #[test]
    fn closing_workspace_with_latest_unread_target_falls_back_to_remaining_unread_surface() {
        let mut app = AppModel::new("C:/work/alpha");
        let alpha = app.selected_workspace().unwrap().id;
        let alpha_surface = app.selected_pane().unwrap().selected_surface;
        let beta = app
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();
        let beta_surface = app.selected_pane().unwrap().selected_surface;

        app.mark_surface_unread(alpha_surface, "Alpha".to_string())
            .unwrap();
        app.mark_surface_unread(beta_surface, "Beta".to_string())
            .unwrap();
        app.close_workspace(beta).unwrap();

        let target = app.latest_unread_target.as_ref().unwrap();
        assert_eq!(target.workspace_id, alpha);
        assert_eq!(target.surface_id, alpha_surface);
        assert_ne!(target.surface_id, beta_surface);
        assert_eq!(target.message, "Alpha");
        assert_eq!(target.sequence, 1);
    }
}

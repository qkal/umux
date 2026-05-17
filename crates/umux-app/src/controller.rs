// SPDX-License-Identifier: GPL-3.0-or-later

use thiserror::Error;
use umux_core::{AppModel, ModelError, PaneId, SurfaceId, SurfaceKind, WorkspaceId};

use crate::{
    AppAction, AppActionOutcome, TerminalRegistry, TerminalRegistryError, TerminalSpawnSpec,
};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum AppControllerError {
    #[error(transparent)]
    Model(#[from] ModelError),
    #[error(transparent)]
    TerminalRegistry(#[from] TerminalRegistryError),
    #[error("no latest unread target")]
    NoLatestUnreadTarget,
}

#[derive(Clone)]
pub struct AppController {
    pub model: AppModel,
    pub terminals: TerminalRegistry,
}

impl AppController {
    pub fn new(model: AppModel) -> Result<Self, AppControllerError> {
        let mut controller = Self {
            model,
            terminals: TerminalRegistry::new(),
        };
        controller.spawn_all_terminal_surfaces()?;
        Ok(controller)
    }

    pub fn from_restored_model(model: AppModel) -> Result<Self, AppControllerError> {
        Self::new(model)
    }

    pub fn apply(&mut self, action: AppAction) -> Result<AppActionOutcome, AppControllerError> {
        let before = self.selected_ids()?;
        let mut outcome = AppActionOutcome {
            should_save_session: true,
            ..AppActionOutcome::default()
        };

        match action {
            AppAction::NewWorkspace { cwd, title } => {
                self.model.create_workspace(cwd, title)?;
                self.spawn_selected_terminal_if_missing(&mut outcome)?;
            }
            AppAction::SelectWorkspace(workspace_id) => {
                self.model.select_workspace(workspace_id)?;
            }
            AppAction::RenameWorkspace {
                workspace_id,
                title,
            } => {
                self.model.rename_workspace(workspace_id, title)?;
            }
            AppAction::RenameSurface { surface_id, title } => {
                self.model.rename_surface(surface_id, title)?;
            }
            AppAction::CloseWorkspace(workspace_id) => {
                let closed_surfaces = self.model.close_workspace(workspace_id)?;
                self.remove_closed_surfaces(closed_surfaces, &mut outcome);
            }
            AppAction::SplitPane(axis) => {
                self.model.split_selected_pane(axis)?;
                self.spawn_selected_terminal_if_missing(&mut outcome)?;
            }
            AppAction::ClosePane(pane_id) => {
                let closed_surfaces = self.model.close_pane(pane_id)?;
                self.remove_closed_surfaces(closed_surfaces, &mut outcome);
            }
            AppAction::SelectPane(pane_id) => {
                self.model.select_pane(pane_id)?;
            }
            AppAction::NewTerminalTab => {
                let surface_id = self.model.open_terminal_surface()?;
                self.spawn_surface(surface_id, &mut outcome)?;
            }
            AppAction::SelectSurface(surface_id) => {
                self.model.select_surface(surface_id)?;
                self.model.mark_surface_read(surface_id)?;
            }
            AppAction::CloseSurface(surface_id) => {
                let closed_surfaces = self.model.close_surface(surface_id)?;
                self.remove_closed_surfaces(closed_surfaces, &mut outcome);
                self.spawn_selected_terminal_if_missing(&mut outcome)?;
            }
            AppAction::JumpLatestUnread => {
                let target = self
                    .model
                    .latest_unread_target
                    .clone()
                    .ok_or(AppControllerError::NoLatestUnreadTarget)?;
                self.model.select_workspace(target.workspace_id)?;
                self.model.select_pane(target.pane_id)?;
                self.model.select_surface(target.surface_id)?;
                self.model.mark_surface_read(target.surface_id)?;
            }
            AppAction::MarkSurfaceRead(surface_id) => {
                self.model.mark_surface_read(surface_id)?;
            }
            AppAction::SaveSessionNow => {}
        }

        let after = self.selected_ids()?;
        outcome.selected_workspace_changed = before.workspace_id != after.workspace_id;
        outcome.selected_pane_changed = before.pane_id != after.pane_id;
        outcome.selected_surface_changed = before.surface_id != after.surface_id;

        Ok(outcome)
    }

    fn spawn_all_terminal_surfaces(&mut self) -> Result<(), AppControllerError> {
        for spec in self.terminal_spawn_specs() {
            self.terminals.spawn(spec)?;
        }
        Ok(())
    }

    fn spawn_surface(
        &mut self,
        surface_id: SurfaceId,
        outcome: &mut AppActionOutcome,
    ) -> Result<(), AppControllerError> {
        let Some(spec) = self
            .terminal_spawn_specs()
            .into_iter()
            .find(|spec| spec.surface_id == surface_id)
        else {
            return Ok(());
        };

        if !self.terminals.contains(surface_id) {
            self.terminals.spawn(spec)?;
            outcome.spawned_surfaces.push(surface_id);
        }

        Ok(())
    }

    fn terminal_spawn_specs(&self) -> Vec<TerminalSpawnSpec> {
        self.model
            .windows
            .iter()
            .flat_map(|window| window.workspaces.iter())
            .flat_map(|workspace| {
                workspace.panes.iter().flat_map(move |pane| {
                    pane.surfaces
                        .iter()
                        .filter(|surface| surface.kind == SurfaceKind::Terminal)
                        .map(move |surface| TerminalSpawnSpec {
                            workspace_id: workspace.id,
                            pane_id: pane.id,
                            surface_id: surface.id,
                            cwd: pane.cwd.clone(),
                        })
                })
            })
            .collect()
    }

    fn spawn_selected_terminal_if_missing(
        &mut self,
        outcome: &mut AppActionOutcome,
    ) -> Result<(), AppControllerError> {
        let pane = self.model.selected_pane()?;
        let surface_id = pane.selected_surface;
        if pane
            .surface(surface_id)
            .is_some_and(|surface| surface.kind == SurfaceKind::Terminal)
        {
            self.spawn_surface(surface_id, outcome)?;
        }
        Ok(())
    }

    fn remove_closed_surfaces(
        &mut self,
        surface_ids: Vec<SurfaceId>,
        outcome: &mut AppActionOutcome,
    ) {
        for surface_id in surface_ids {
            self.terminals.remove(surface_id);
            outcome.closed_surfaces.push(surface_id);
        }
    }

    fn selected_ids(&self) -> Result<SelectedIds, AppControllerError> {
        let workspace = self.model.selected_workspace()?;
        let pane = workspace
            .selected_pane()
            .ok_or(ModelError::NoSelectedPane)?;

        Ok(SelectedIds {
            workspace_id: workspace.id,
            pane_id: pane.id,
            surface_id: pane.selected_surface,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SelectedIds {
    workspace_id: WorkspaceId,
    pane_id: PaneId,
    surface_id: SurfaceId,
}

#[cfg(test)]
mod tests {
    use umux_core::{AppModel, SplitAxis};

    use crate::{AppAction, AppController, AppControllerError};

    #[test]
    fn controller_spawns_initial_terminal() {
        let controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let surface_id = controller.model.selected_pane().unwrap().selected_surface;

        assert!(controller.terminals.contains(surface_id));
        assert_eq!(controller.terminals.len(), 1);
    }

    #[test]
    fn new_workspace_selects_workspace_and_spawns_terminal() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();

        let outcome = controller
            .apply(AppAction::NewWorkspace {
                cwd: "C:/work/beta".to_string(),
                title: Some("Beta".to_string()),
            })
            .unwrap();

        let workspace = controller.model.selected_workspace().unwrap();
        let surface_id = workspace.selected_pane().unwrap().selected_surface;
        assert_eq!(workspace.title, "Beta");
        assert!(outcome.selected_workspace_changed);
        assert!(outcome.selected_pane_changed);
        assert!(outcome.selected_surface_changed);
        assert_eq!(outcome.spawned_surfaces, vec![surface_id]);
        assert!(controller.terminals.contains(surface_id));
        assert_eq!(controller.terminals.len(), 2);
    }

    #[test]
    fn controller_renames_surface() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let surface_id = controller.model.selected_pane().unwrap().selected_surface;

        controller
            .apply(AppAction::RenameSurface {
                surface_id,
                title: "cargo test".to_string(),
            })
            .unwrap();

        let surface = controller
            .model
            .selected_pane()
            .unwrap()
            .surface(surface_id)
            .unwrap();
        assert_eq!(surface.title, "cargo test");
    }

    #[test]
    fn closing_workspace_removes_owned_terminal_sessions() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let alpha = controller.model.selected_workspace().unwrap().id;
        controller
            .apply(AppAction::NewWorkspace {
                cwd: "C:/work/beta".to_string(),
                title: Some("Beta".to_string()),
            })
            .unwrap();
        let beta = controller.model.selected_workspace().unwrap().id;
        let beta_surface = controller.model.selected_pane().unwrap().selected_surface;

        let outcome = controller.apply(AppAction::CloseWorkspace(beta)).unwrap();

        assert_eq!(controller.model.selected_workspace().unwrap().id, alpha);
        assert_eq!(outcome.closed_surfaces, vec![beta_surface]);
        assert!(!controller.terminals.contains(beta_surface));
        assert_eq!(controller.terminals.len(), 1);
    }

    #[test]
    fn split_pane_spawns_new_terminal_session() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();

        let outcome = controller
            .apply(AppAction::SplitPane(SplitAxis::Vertical))
            .unwrap();

        let pane = controller.model.selected_pane().unwrap();
        let surface_id = pane.selected_surface;
        assert!(outcome.selected_pane_changed);
        assert!(outcome.selected_surface_changed);
        assert_eq!(outcome.spawned_surfaces, vec![surface_id]);
        assert!(controller.terminals.contains(surface_id));
        assert_eq!(controller.terminals.len(), 2);
    }

    #[test]
    fn jump_latest_unread_selects_target() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let alpha = controller.model.selected_workspace().unwrap().id;
        controller
            .apply(AppAction::NewWorkspace {
                cwd: "C:/work/beta".to_string(),
                title: Some("Beta".to_string()),
            })
            .unwrap();
        let beta = controller.model.selected_workspace().unwrap().id;
        let beta_pane = controller.model.selected_pane().unwrap().id;
        let beta_surface = controller.model.selected_pane().unwrap().selected_surface;
        controller.model.select_workspace(alpha).unwrap();
        controller
            .model
            .mark_surface_unread(beta_surface, "done".to_string())
            .unwrap();

        let outcome = controller.apply(AppAction::JumpLatestUnread).unwrap();

        assert_eq!(controller.model.selected_workspace().unwrap().id, beta);
        assert_eq!(controller.model.selected_pane().unwrap().id, beta_pane);
        assert_eq!(
            controller.model.selected_pane().unwrap().selected_surface,
            beta_surface
        );
        assert!(outcome.selected_workspace_changed);
        assert!(outcome.selected_pane_changed);
        assert!(outcome.selected_surface_changed);
    }

    #[test]
    fn selecting_unread_surface_marks_it_read() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let first = controller.model.selected_pane().unwrap().selected_surface;
        controller.apply(AppAction::NewTerminalTab).unwrap();
        controller
            .model
            .mark_surface_unread(first, "done".to_string())
            .unwrap();

        controller.apply(AppAction::SelectSurface(first)).unwrap();

        let surface = controller
            .model
            .selected_pane()
            .unwrap()
            .surface(first)
            .unwrap();
        assert!(!surface.unread);
        assert_eq!(controller.model.latest_unread_target, None);
    }

    #[test]
    fn jump_latest_unread_without_target_returns_error() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();

        let result = controller.apply(AppAction::JumpLatestUnread);

        assert_eq!(result, Err(AppControllerError::NoLatestUnreadTarget));
    }

    #[test]
    fn closing_latest_unread_surface_clears_or_falls_back_target() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let first = controller.model.selected_pane().unwrap().selected_surface;
        controller.apply(AppAction::NewTerminalTab).unwrap();
        let second = controller.model.selected_pane().unwrap().selected_surface;
        controller
            .model
            .mark_surface_unread(second, "done".to_string())
            .unwrap();

        controller.apply(AppAction::CloseSurface(second)).unwrap();

        assert_ne!(
            controller
                .model
                .latest_unread_target
                .as_ref()
                .map(|target| target.surface_id),
            Some(second)
        );
        assert!(controller.terminals.contains(first));
    }

    #[test]
    fn close_surface_registers_replacement_terminal_when_pane_would_be_empty() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let closed = controller.model.selected_pane().unwrap().selected_surface;

        let outcome = controller.apply(AppAction::CloseSurface(closed)).unwrap();

        let replacement = controller.model.selected_pane().unwrap().selected_surface;
        assert_ne!(replacement, closed);
        assert_eq!(outcome.closed_surfaces, vec![closed]);
        assert_eq!(outcome.spawned_surfaces, vec![replacement]);
        assert!(!controller.terminals.contains(closed));
        assert!(controller.terminals.contains(replacement));
        assert_eq!(controller.terminals.len(), 1);
    }

    #[test]
    fn successful_actions_request_session_save() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let selected_workspace = controller.model.selected_workspace().unwrap().id;

        let save_now = controller.apply(AppAction::SaveSessionNow).unwrap();
        let select_workspace = controller
            .apply(AppAction::SelectWorkspace(selected_workspace))
            .unwrap();

        assert!(save_now.should_save_session);
        assert!(select_workspace.should_save_session);
    }

    #[test]
    fn controller_restore_spawns_restored_terminals() {
        let mut model = AppModel::new("C:/work/alpha");
        model
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();

        let controller = AppController::from_restored_model(model).unwrap();

        assert_eq!(controller.terminals.len(), 2);
        assert_eq!(controller.model.selected_workspace().unwrap().title, "Beta");
    }
}

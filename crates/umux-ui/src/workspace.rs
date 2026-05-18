// SPDX-License-Identifier: GPL-3.0-or-later

use std::{sync::Arc, time::Duration};

use gpui::{App, Context, IntoElement, Render, Task, WeakEntity, Window, div, prelude::*, px};
use umux_app::{
    AppAction, AppController, AppControllerError, SessionStore, TerminalEntry,
    TerminalEntrySnapshot,
};
use umux_core::{PaneId, SurfaceId, SurfaceKind, WorkspaceId};
use umux_terminal::TerminalStatus;
use umux_ui_kit::theme::{BACKGROUND, MUTED_TEXT, PANEL, TEXT};

use crate::actions;
use crate::shell::{pane_group, top_bar, workspace_rail};
use crate::startup::StartupState;
use crate::terminal::TerminalSurfaceState;
use crate::view_model::workspace_rows;

pub struct UmuxWorkspace {
    pub controller: AppController,
    pub store: Arc<SessionStore>,
    pub startup_warning: Option<String>,
    terminal_surface_state: TerminalSurfaceState,
    terminal_refresh_state: Vec<TerminalRefreshEntry>,
    _terminal_refresh_task: Option<Task<()>>,
}

impl UmuxWorkspace {
    pub fn new(startup: StartupState, store: SessionStore) -> Self {
        let mut workspace = Self {
            controller: startup.controller,
            store: Arc::new(store),
            startup_warning: startup.warning,
            terminal_surface_state: TerminalSurfaceState::new(),
            terminal_refresh_state: Vec::new(),
            _terminal_refresh_task: None,
        };
        workspace.materialize_visible_terminals();
        workspace.terminal_refresh_state = workspace.terminal_refresh_state();
        workspace
    }

    pub fn new_with_context(
        startup: StartupState,
        store: SessionStore,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut workspace = Self::new(startup, store);
        workspace.start_terminal_refresh_task(cx);
        workspace
    }

    pub fn selected_workspace_title(&self) -> String {
        self.controller
            .model
            .selected_workspace()
            .map(|workspace| workspace.title.clone())
            .unwrap_or_else(|_| "Workspace".to_string())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn dispatch(
        &mut self,
        action: umux_app::AppAction,
    ) -> Result<umux_app::AppActionOutcome, umux_app::AppControllerError> {
        let outcome = self.controller.apply(action)?;
        self.materialize_visible_terminals();
        if outcome.should_save_session
            && let Err(error) = self.store.save_model(&self.controller.model)
        {
            tracing::warn!(%error, "failed to save session");
        }
        Ok(outcome)
    }

    pub(crate) fn dispatch_many(
        &mut self,
        actions: impl IntoIterator<Item = AppAction>,
    ) -> Result<(), AppControllerError> {
        for action in actions {
            self.dispatch(action)?;
        }
        Ok(())
    }

    fn dispatch_and_notify(&mut self, action: AppAction, cx: &mut Context<Self>) {
        self.dispatch_many_and_notify([action], cx);
    }

    fn dispatch_many_and_notify(
        &mut self,
        actions: impl IntoIterator<Item = AppAction>,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.dispatch_many(actions) {
            tracing::warn!(%error, "failed to dispatch workspace action");
        }
        cx.notify();
    }

    fn dispatch_from_weak(handle: &WeakEntity<Self>, action: AppAction, cx: &mut App) {
        Self::dispatch_actions_from_weak(handle, vec![action], cx);
    }

    fn dispatch_actions_from_weak(
        handle: &WeakEntity<Self>,
        actions: Vec<AppAction>,
        cx: &mut App,
    ) {
        if let Err(error) = handle.update(cx, move |workspace, cx| {
            workspace.dispatch_many_and_notify(actions, cx);
        }) {
            tracing::debug!(%error, "workspace interaction target was released");
        }
    }

    fn materialize_visible_terminals(&mut self) {
        if let Err(error) = self.controller.materialize_visible_terminal_surfaces() {
            tracing::warn!(%error, "failed to materialize visible terminal surfaces");
        }
    }

    fn start_terminal_refresh_task(&mut self, cx: &mut Context<Self>) {
        self._terminal_refresh_task = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                let update_result = this.update(cx, |workspace, cx| {
                    let notifications_changed = workspace.drain_terminal_notifications();
                    let next = workspace.terminal_refresh_state();
                    if workspace.terminal_refresh_state != next || notifications_changed {
                        workspace.terminal_refresh_state = next;
                        cx.notify();
                    }
                });

                if let Err(error) = update_result {
                    tracing::debug!(%error, "terminal refresh task stopped");
                    break;
                }
            }
        }));
    }

    fn terminal_refresh_state(&self) -> Vec<TerminalRefreshEntry> {
        let Ok(workspace) = self.controller.model.selected_workspace() else {
            return Vec::new();
        };

        workspace
            .panes
            .iter()
            .filter_map(|pane| {
                let surface = pane.surface(pane.selected_surface)?;
                (surface.kind == SurfaceKind::Terminal).then(|| {
                    terminal_refresh_entry(surface.id, self.controller.terminals.entry(surface.id))
                })
            })
            .collect()
    }

    fn drain_terminal_notifications(&mut self) -> bool {
        let surface_ids = self.controller.terminals.running_surface_ids();
        let mut changed = false;

        for surface_id in surface_ids {
            if let Some(entry) = self.controller.terminals.entry(surface_id) {
                for notification in entry.drain_notifications() {
                    changed |= self
                        .controller
                        .model
                        .mark_surface_unread(surface_id, notification.message)
                        .is_ok();
                }
            }
        }

        if changed && let Err(error) = self.store.save_model(&self.controller.model) {
            tracing::warn!(%error, "failed to save session after terminal notification");
        }

        changed
    }

    fn on_new_workspace(
        &mut self,
        _: &actions::NewWorkspace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_and_notify(
            AppAction::NewWorkspace {
                cwd: crate::startup::current_dir_cwd(),
                title: None,
            },
            cx,
        );
    }

    fn on_new_terminal_tab(
        &mut self,
        _: &actions::NewTerminalTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_and_notify(AppAction::NewTerminalTab, cx);
    }

    fn on_close_terminal_tab(
        &mut self,
        _: &actions::CloseTerminalTab,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(action) = actions::close_surface_action(&self.controller.model) {
            self.dispatch_and_notify(action, cx);
        } else {
            cx.notify();
        }
    }

    fn on_close_workspace(
        &mut self,
        _: &actions::CloseWorkspace,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(action) = actions::close_workspace_action(&self.controller.model) {
            self.dispatch_and_notify(action, cx);
        } else {
            cx.notify();
        }
    }

    fn on_split_right(
        &mut self,
        _: &actions::SplitRight,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_and_notify(actions::split_right_action(), cx);
    }

    fn on_split_down(
        &mut self,
        _: &actions::SplitDown,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_and_notify(actions::split_down_action(), cx);
    }

    fn on_jump_latest_unread(
        &mut self,
        _: &actions::JumpLatestUnread,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dispatch_and_notify(AppAction::JumpLatestUnread, cx);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalRefreshEntry {
    surface_id: SurfaceId,
    registered: bool,
    failed_message: Option<String>,
    status: Option<TerminalStatus>,
    last_error: Option<String>,
    snapshot_version: Option<u64>,
}

fn terminal_refresh_entry(
    surface_id: SurfaceId,
    entry: Option<&TerminalEntry>,
) -> TerminalRefreshEntry {
    match entry {
        Some(TerminalEntry::Failed { message, .. }) => TerminalRefreshEntry {
            surface_id,
            registered: true,
            failed_message: Some(message.clone()),
            status: None,
            last_error: None,
            snapshot_version: None,
        },
        Some(entry) => {
            terminal_refresh_entry_from_snapshot(surface_id, true, None, entry.snapshot_state())
        }
        None => TerminalRefreshEntry {
            surface_id,
            registered: false,
            failed_message: None,
            status: None,
            last_error: None,
            snapshot_version: None,
        },
    }
}

fn terminal_refresh_entry_from_snapshot(
    surface_id: SurfaceId,
    registered: bool,
    failed_message: Option<String>,
    snapshot: TerminalEntrySnapshot,
) -> TerminalRefreshEntry {
    TerminalRefreshEntry {
        surface_id,
        registered,
        failed_message,
        status: snapshot.health.as_ref().map(|health| health.status.clone()),
        last_error: snapshot.health.and_then(|health| health.last_error),
        snapshot_version: snapshot
            .renderer_snapshot
            .as_ref()
            .map(|snapshot| snapshot.version),
    }
}

impl Render for UmuxWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.selected_workspace_title();
        let warning = self.startup_warning.clone();
        let workspace_handle = cx.weak_entity();
        let on_select_workspace = {
            let workspace_handle = workspace_handle.clone();
            move |workspace_id: WorkspaceId, cx: &mut App| {
                Self::dispatch_from_weak(
                    &workspace_handle,
                    AppAction::SelectWorkspace(workspace_id),
                    cx,
                );
            }
        };
        let on_new_workspace = {
            let workspace_handle = workspace_handle.clone();
            move |cx: &mut App| {
                Self::dispatch_from_weak(
                    &workspace_handle,
                    AppAction::NewWorkspace {
                        cwd: crate::startup::current_dir_cwd(),
                        title: None,
                    },
                    cx,
                );
            }
        };
        let on_select_surface = {
            let workspace_handle = workspace_handle.clone();
            move |pane_id: PaneId, surface_id: SurfaceId, cx: &mut App| {
                Self::dispatch_actions_from_weak(
                    &workspace_handle,
                    vec![
                        AppAction::SelectPane(pane_id),
                        AppAction::SelectSurface(surface_id),
                    ],
                    cx,
                );
            }
        };
        let on_close_surface = {
            let workspace_handle = workspace_handle.clone();
            move |pane_id: PaneId, surface_id: SurfaceId, cx: &mut App| {
                Self::dispatch_actions_from_weak(
                    &workspace_handle,
                    vec![
                        AppAction::SelectPane(pane_id),
                        AppAction::CloseSurface(surface_id),
                    ],
                    cx,
                );
            }
        };
        let on_new_surface = {
            let workspace_handle = workspace_handle.clone();
            move |pane_id: PaneId, cx: &mut App| {
                Self::dispatch_actions_from_weak(
                    &workspace_handle,
                    vec![AppAction::SelectPane(pane_id), AppAction::NewTerminalTab],
                    cx,
                );
            }
        };
        let body = self
            .controller
            .model
            .selected_window()
            .ok()
            .map(|window| {
                let rows = workspace_rows(&window.workspaces, window.selected_workspace);
                let selected_workspace = window.selected_workspace().cloned();

                div()
                    .flex()
                    .flex_1()
                    .w_full()
                    .child(workspace_rail(
                        rows,
                        on_select_workspace.clone(),
                        on_new_workspace.clone(),
                    ))
                    .child(
                        selected_workspace
                            .as_ref()
                            .map(|workspace| {
                                pane_group(
                                    &self.controller,
                                    workspace,
                                    &self.terminal_surface_state,
                                    on_select_surface.clone(),
                                    on_close_surface.clone(),
                                    on_new_surface.clone(),
                                )
                            })
                            .unwrap_or_else(|| {
                                div()
                                    .flex()
                                    .flex_1()
                                    .items_center()
                                    .justify_center()
                                    .text_color(MUTED_TEXT)
                                    .child("missing selected workspace")
                            }),
                    )
            })
            .unwrap_or_else(|| {
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_color(MUTED_TEXT)
                    .child("missing selected window")
            });

        div()
            .on_action(cx.listener(Self::on_new_workspace))
            .on_action(cx.listener(Self::on_new_terminal_tab))
            .on_action(cx.listener(Self::on_close_terminal_tab))
            .on_action(cx.listener(Self::on_close_workspace))
            .on_action(cx.listener(Self::on_split_right))
            .on_action(cx.listener(Self::on_split_down))
            .on_action(cx.listener(Self::on_jump_latest_unread))
            .flex()
            .flex_col()
            .size_full()
            .bg(BACKGROUND)
            .text_color(TEXT)
            .child(top_bar(title))
            .when_some(warning, |shell, warning| {
                shell.child(
                    div()
                        .w_full()
                        .h(px(30.0))
                        .px(px(14.0))
                        .flex()
                        .items_center()
                        .bg(PANEL)
                        .text_color(MUTED_TEXT)
                        .text_size(px(12.0))
                        .child(warning),
                )
            })
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use camino::Utf8PathBuf;
    use umux_app::{AppAction, AppController, SessionStore, TerminalEntrySnapshot};
    use umux_core::{AppModel, SurfaceId, model::SplitAxis};
    use umux_terminal::{TerminalCursor, TerminalHealth, TerminalRendererSnapshot, TerminalStatus};

    use crate::startup::StartupState;

    use super::UmuxWorkspace;

    #[test]
    fn dispatch_persists_successful_workspace_update() {
        let temp_dir = TempSessionDir::new("dispatch-persist");
        let session_path = temp_dir.path.join("session.json");
        let store = SessionStore::new(session_path.clone());
        let startup = StartupState {
            controller: AppController::new_deferred_terminals(AppModel::new("C:/work/alpha"))
                .unwrap(),
            warning: None,
        };
        let mut workspace = UmuxWorkspace::new(startup, store);

        let outcome = workspace
            .dispatch(AppAction::NewWorkspace {
                cwd: "C:/work/beta".to_string(),
                title: Some("Beta".to_string()),
            })
            .unwrap();

        let loaded = SessionStore::new(session_path)
            .load_model()
            .unwrap()
            .unwrap();
        let selected_workspace = loaded.selected_workspace().unwrap();
        assert!(outcome.should_save_session);
        assert_eq!(selected_workspace.title, "Beta");
        assert_eq!(selected_workspace.cwd, "C:/work/beta");
    }

    #[test]
    fn workspace_materializes_initial_deferred_terminal() {
        let temp_dir = TempSessionDir::new("materialize-initial");
        let store = SessionStore::new(temp_dir.path.join("session.json"));
        let startup = StartupState {
            controller: AppController::new_deferred_terminals(AppModel::new("C:/work/alpha"))
                .unwrap(),
            warning: None,
        };
        let surface_id = startup
            .controller
            .model
            .selected_pane()
            .unwrap()
            .selected_surface;

        let workspace = UmuxWorkspace::new(startup, store);

        assert!(workspace.controller.terminals.contains(surface_id));
        assert_eq!(workspace.controller.terminals.len(), 1);
    }

    #[test]
    fn dispatch_many_selects_surface_in_target_split_pane() {
        let mut model = AppModel::new("C:/work/alpha");
        let first_pane = model.selected_pane().unwrap().id;
        let first_surface = model.selected_pane().unwrap().selected_surface;
        let second_pane = model.split_selected_pane(SplitAxis::Vertical).unwrap();
        assert_eq!(model.selected_pane().unwrap().id, second_pane);

        let (_temp_dir, mut workspace) = workspace_from_model("split-select-surface", model);

        workspace
            .dispatch_many([
                AppAction::SelectPane(first_pane),
                AppAction::SelectSurface(first_surface),
            ])
            .unwrap();

        let selected_pane = workspace.controller.model.selected_pane().unwrap();
        assert_eq!(selected_pane.id, first_pane);
        assert_eq!(selected_pane.selected_surface, first_surface);
    }

    #[test]
    fn dispatch_many_closes_surface_in_target_split_pane() {
        let mut model = AppModel::new("C:/work/alpha");
        let first_pane = model.selected_pane().unwrap().id;
        let first_surface = model.selected_pane().unwrap().selected_surface;
        let surface_to_close = model.open_terminal_surface().unwrap();
        let second_pane = model.split_selected_pane(SplitAxis::Vertical).unwrap();
        assert_eq!(model.selected_pane().unwrap().id, second_pane);

        let (_temp_dir, mut workspace) = workspace_from_model("split-close-surface", model);

        workspace
            .dispatch_many([
                AppAction::SelectPane(first_pane),
                AppAction::CloseSurface(surface_to_close),
            ])
            .unwrap();

        let selected_workspace = workspace.controller.model.selected_workspace().unwrap();
        let first = selected_workspace.pane(first_pane).unwrap();
        assert_eq!(selected_workspace.selected_pane, first_pane);
        assert_eq!(first.selected_surface, first_surface);
        assert!(
            !first
                .surfaces
                .iter()
                .any(|surface| surface.id == surface_to_close)
        );
    }

    #[test]
    fn dispatch_many_opens_terminal_tab_in_target_split_pane() {
        let mut model = AppModel::new("C:/work/alpha");
        let first_pane = model.selected_pane().unwrap().id;
        let first_count = model.selected_pane().unwrap().surfaces.len();
        let second_pane = model.split_selected_pane(SplitAxis::Vertical).unwrap();
        let second_count = model.selected_pane().unwrap().surfaces.len();
        assert_eq!(model.selected_pane().unwrap().id, second_pane);

        let (_temp_dir, mut workspace) = workspace_from_model("split-new-terminal-tab", model);

        workspace
            .dispatch_many([AppAction::SelectPane(first_pane), AppAction::NewTerminalTab])
            .unwrap();

        let selected_workspace = workspace.controller.model.selected_workspace().unwrap();
        let first = selected_workspace.pane(first_pane).unwrap();
        let second = selected_workspace.pane(second_pane).unwrap();
        assert_eq!(selected_workspace.selected_pane, first_pane);
        assert_eq!(first.surfaces.len(), first_count + 1);
        assert_eq!(second.surfaces.len(), second_count);
        assert_eq!(
            first.selected_surface,
            first.surfaces.last().expect("new terminal surface").id
        );
    }

    #[test]
    fn terminal_refresh_state_tracks_snapshot_status_and_last_error_changes() {
        let mut health = TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24);
        health.status = TerminalStatus::Failed;
        health.last_error = Some("read failed".to_string());

        let entry = super::terminal_refresh_entry_from_snapshot(
            SurfaceId(10),
            true,
            None,
            TerminalEntrySnapshot {
                health: Some(health),
                renderer_snapshot: Some(snapshot(7)),
            },
        );

        assert_eq!(entry.surface_id, SurfaceId(10));
        assert_eq!(entry.status, Some(TerminalStatus::Failed));
        assert_eq!(entry.last_error, Some("read failed".to_string()));
        assert_eq!(entry.snapshot_version, Some(7));
    }

    #[test]
    fn terminal_refresh_state_tracks_failed_registry_entry_message() {
        let entry = umux_app::TerminalEntry::Failed {
            spec: umux_app::TerminalSpawnSpec {
                workspace_id: umux_core::WorkspaceId(1),
                pane_id: umux_core::PaneId(2),
                surface_id: SurfaceId(10),
                cwd: "C:/work/alpha".to_string(),
            },
            message: "pty refused startup".to_string(),
        };

        let state = super::terminal_refresh_entry(SurfaceId(10), Some(&entry));

        assert!(state.registered);
        assert_eq!(
            state.failed_message,
            Some("pty refused startup".to_string())
        );
    }

    fn snapshot(version: u64) -> TerminalRendererSnapshot {
        TerminalRendererSnapshot {
            cols: 1,
            rows: 1,
            cells: Vec::new(),
            cursor: TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection: None,
            scrollback_lines: 0,
            version,
        }
    }

    struct TempSessionDir {
        path: Utf8PathBuf,
    }

    impl TempSessionDir {
        fn new(name: &str) -> Self {
            let nanos = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir()
                .join("umux-ui-workspace-tests")
                .join(format!("{name}-{nanos}-{}", std::process::id()));
            fs::remove_dir_all(&path).ok();
            Self {
                path: Utf8PathBuf::from_path_buf(path).unwrap(),
            }
        }
    }

    fn workspace_from_model(name: &str, model: AppModel) -> (TempSessionDir, UmuxWorkspace) {
        let temp_dir = TempSessionDir::new(name);
        let store = SessionStore::new(temp_dir.path.join("session.json"));
        let startup = StartupState {
            controller: AppController::new_deferred_terminals(model).unwrap(),
            warning: None,
        };
        let workspace = UmuxWorkspace::new(startup, store);
        (temp_dir, workspace)
    }

    impl Drop for TempSessionDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).ok();
        }
    }
}

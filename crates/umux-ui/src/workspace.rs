// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use umux_app::{AppAction, AppController, SessionStore};
use umux_ui_kit::theme::{BACKGROUND, MUTED_TEXT, PANEL, TEXT};

use crate::actions;
use crate::startup::StartupState;

pub struct UmuxWorkspace {
    pub controller: AppController,
    pub store: Arc<SessionStore>,
    pub startup_warning: Option<String>,
}

impl UmuxWorkspace {
    pub fn new(startup: StartupState, store: SessionStore) -> Self {
        Self {
            controller: startup.controller,
            store: Arc::new(store),
            startup_warning: startup.warning,
        }
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
        if outcome.should_save_session {
            if let Err(error) = self.store.save_model(&self.controller.model) {
                tracing::warn!(%error, "failed to save session");
            }
        }
        Ok(outcome)
    }

    fn dispatch_and_notify(&mut self, action: AppAction, cx: &mut Context<Self>) {
        if let Err(error) = self.dispatch(action) {
            tracing::warn!(%error, "failed to dispatch workspace action");
        }
        cx.notify();
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

impl Render for UmuxWorkspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.selected_workspace_title();
        let warning = self.startup_warning.clone();

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
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .h(px(40.0))
                    .px(px(14.0))
                    .bg(PANEL)
                    .text_size(px(12.0))
                    .child(div().font_weight(gpui::FontWeight::BOLD).child("umux"))
                    .child(div().text_color(MUTED_TEXT).child(title)),
            )
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
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .text_color(MUTED_TEXT)
                    .child("GPUI workspace shell"),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use camino::Utf8PathBuf;
    use umux_app::{AppAction, AppController, SessionStore};
    use umux_core::AppModel;

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

    impl Drop for TempSessionDir {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.path).ok();
        }
    }
}

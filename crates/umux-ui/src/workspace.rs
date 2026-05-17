// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;

use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px};
use umux_app::{AppController, SessionStore};
use umux_ui_kit::theme::{BACKGROUND, MUTED_TEXT, PANEL, TEXT};

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
}

impl Render for UmuxWorkspace {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.selected_workspace_title();
        let warning = self.startup_warning.clone();

        div()
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

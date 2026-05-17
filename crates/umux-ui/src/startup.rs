// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;

use tracing::{info, warn};
use umux_app::session_store::SessionLoadOutcome;
use umux_app::{AppController, AppControllerError, SessionStore, SessionStoreError};
use umux_core::AppModel;

const RECOVERED_SESSION_WARNING: &str =
    "Previous session could not be restored. A recovered copy was moved aside.";
const SESSION_READ_WARNING: &str = "Session file could not be read. Opened a fresh workspace.";
const RESTORE_CONTROLLER_WARNING: &str =
    "Previous session could not be restored. Opened a fresh workspace.";

#[derive(Clone)]
pub struct StartupState {
    pub controller: AppController,
    pub warning: Option<String>,
}

pub fn current_dir_cwd() -> String {
    env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

pub fn seed_model() -> AppModel {
    AppModel::new(current_dir_cwd())
}

pub fn startup_state_from_store(store: &SessionStore) -> StartupState {
    startup_state_from_decision(startup_load_decision(
        store.load_model_with_status(),
        current_dir_cwd(),
    ))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StartupLoadDecision {
    model: AppModel,
    restored: bool,
    warning: Option<String>,
}

fn startup_load_decision(
    load_result: Result<SessionLoadOutcome, SessionStoreError>,
    fallback_cwd: String,
) -> StartupLoadDecision {
    match load_result {
        Ok(SessionLoadOutcome::Missing) => {
            info!(%fallback_cwd, "no saved session found; starting fresh workspace");
            StartupLoadDecision {
                model: AppModel::new(fallback_cwd),
                restored: false,
                warning: None,
            }
        }
        Ok(SessionLoadOutcome::Loaded(model)) => {
            info!("saved session loaded");
            StartupLoadDecision {
                model,
                restored: true,
                warning: None,
            }
        }
        Ok(SessionLoadOutcome::RecoveredCorrupt { corrupt_path }) => {
            warn!(%corrupt_path, "corrupt saved session recovered");
            StartupLoadDecision {
                model: AppModel::new(fallback_cwd),
                restored: false,
                warning: Some(RECOVERED_SESSION_WARNING.to_string()),
            }
        }
        Err(error) => {
            warn!(%error, "saved session could not be read");
            StartupLoadDecision {
                model: AppModel::new(fallback_cwd),
                restored: false,
                warning: Some(SESSION_READ_WARNING.to_string()),
            }
        }
    }
}

fn startup_state_from_decision(decision: StartupLoadDecision) -> StartupState {
    match AppController::from_restored_model_deferred_terminals(decision.model) {
        Ok(controller) => {
            if decision.restored {
                info!("restored saved session controller");
            } else {
                info!("started fresh workspace controller");
            }
            StartupState {
                controller,
                warning: decision.warning,
            }
        }
        Err(error) => fresh_startup_after_restore_error(error),
    }
}

fn fresh_startup_after_restore_error(error: AppControllerError) -> StartupState {
    warn!(%error, "restored session could not initialize controller");
    StartupState {
        controller: AppController::new_deferred_terminals(seed_model())
            .expect("seed model should create an app controller"),
        warning: Some(RESTORE_CONTROLLER_WARNING.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error, ErrorKind};
    use umux_core::SplitAxis;

    #[test]
    fn seed_model_uses_a_nonempty_cwd() {
        let model = seed_model();

        assert!(!model.selected_workspace().unwrap().cwd.is_empty());
    }

    #[test]
    fn startup_load_decision_seeds_without_warning_when_session_is_missing() {
        let decision =
            startup_load_decision(Ok(SessionLoadOutcome::Missing), "C:/work/fresh".to_string());

        assert!(!decision.restored);
        assert_eq!(decision.warning, None);
        assert_eq!(
            decision.model.selected_workspace().unwrap().cwd,
            "C:/work/fresh"
        );
    }

    #[test]
    fn startup_load_decision_warns_when_corrupt_session_was_recovered() {
        let mut corrupt_path = SessionStore::default_path();
        corrupt_path.set_file_name("session.json.corrupt.test");
        let decision = startup_load_decision(
            Ok(SessionLoadOutcome::RecoveredCorrupt { corrupt_path }),
            "C:/work/fresh".to_string(),
        );

        assert!(!decision.restored);
        assert_eq!(
            decision.warning,
            Some(RECOVERED_SESSION_WARNING.to_string())
        );
        assert_eq!(
            decision.model.selected_workspace().unwrap().cwd,
            "C:/work/fresh"
        );
    }

    #[test]
    fn startup_load_decision_warns_when_session_file_cannot_be_read() {
        let decision = startup_load_decision(
            Err(SessionStoreError::Io(Error::new(
                ErrorKind::PermissionDenied,
                "denied",
            ))),
            "C:/work/fresh".to_string(),
        );

        assert!(!decision.restored);
        assert_eq!(decision.warning, Some(SESSION_READ_WARNING.to_string()));
        assert_eq!(
            decision.model.selected_workspace().unwrap().cwd,
            "C:/work/fresh"
        );
    }

    #[test]
    fn startup_load_decision_marks_loaded_session_as_restored() {
        let model = AppModel::new("C:/work/restored");
        let decision = startup_load_decision(
            Ok(SessionLoadOutcome::Loaded(model.clone())),
            "C:/work/fresh".to_string(),
        );

        assert!(decision.restored);
        assert_eq!(decision.warning, None);
        assert_eq!(decision.model, model);
    }

    #[test]
    fn startup_state_defers_terminal_spawn_for_gpui_shell() {
        let startup = startup_state_from_decision(StartupLoadDecision {
            model: AppModel::new("C:/work/restored"),
            restored: true,
            warning: None,
        });

        let surface_id = startup
            .controller
            .model
            .selected_pane()
            .unwrap()
            .selected_surface;
        assert!(!startup.controller.terminals.contains(surface_id));
        assert_eq!(startup.controller.terminals.len(), 0);
    }

    #[test]
    fn startup_state_falls_back_when_loaded_session_has_duplicate_terminal_ids() {
        let mut model = AppModel::new("C:/work/restored");
        let duplicate_surface = model.selected_pane().unwrap().surfaces[0].clone();
        model.split_selected_pane(SplitAxis::Vertical).unwrap();
        model
            .selected_pane_mut()
            .unwrap()
            .surfaces
            .push(duplicate_surface);

        let startup = startup_state_from_decision(StartupLoadDecision {
            model,
            restored: true,
            warning: None,
        });

        assert_eq!(
            startup.warning,
            Some(RESTORE_CONTROLLER_WARNING.to_string())
        );
        assert_eq!(
            startup.controller.model.selected_workspace().unwrap().cwd,
            current_dir_cwd()
        );
        assert_eq!(startup.controller.terminals.len(), 0);
    }
}

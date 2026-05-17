// SPDX-License-Identifier: GPL-3.0-or-later

use umux_app::{TerminalEntry, TerminalEntrySnapshot};
use umux_terminal::{TerminalHealth, TerminalRendererSnapshot, TerminalStatus};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalBridgeState {
    pub status: String,
    pub snapshot: Option<TerminalRendererSnapshot>,
    pub failed: bool,
}

pub fn state_from_entry(entry: Option<&TerminalEntry>) -> TerminalBridgeState {
    match entry {
        Some(TerminalEntry::Failed { message, .. }) => TerminalBridgeState {
            status: format!("terminal failed: {message}"),
            snapshot: None,
            failed: true,
        },
        Some(entry) => state_from_terminal_read(Some(entry.snapshot_state())),
        None => TerminalBridgeState {
            status: "terminal missing".to_string(),
            snapshot: None,
            failed: true,
        },
    }
}

fn state_from_terminal_read(read: Option<TerminalEntrySnapshot>) -> TerminalBridgeState {
    let Some(read) = read else {
        return TerminalBridgeState {
            status: "terminal unavailable".to_string(),
            snapshot: None,
            failed: false,
        };
    };

    let failed = read.health.as_ref().is_some_and(|health| {
        matches!(
            health.status,
            TerminalStatus::Exited | TerminalStatus::Failed
        )
    });
    let status = read
        .health
        .as_ref()
        .map(health_status)
        .unwrap_or_else(|| "terminal unavailable".to_string());

    TerminalBridgeState {
        status,
        snapshot: read.renderer_snapshot,
        failed,
    }
}

fn health_status(health: &TerminalHealth) -> String {
    match health.status {
        TerminalStatus::Starting => {
            format!("{} {}x{} starting", health.shell, health.cols, health.rows)
        }
        TerminalStatus::Running => {
            format!("{} {}x{} running", health.shell, health.cols, health.rows)
        }
        TerminalStatus::Exited => {
            format!("{} {}x{} exited", health.shell, health.cols, health.rows)
        }
        TerminalStatus::Failed => health
            .last_error
            .as_ref()
            .map(|last_error| format!("terminal failed: {last_error}"))
            .unwrap_or_else(|| format!("{} {}x{} failed", health.shell, health.cols, health.rows)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_app::TerminalSpawnSpec;
    use umux_core::{PaneId, SurfaceId, WorkspaceId};

    fn spec(surface_id: u64) -> TerminalSpawnSpec {
        TerminalSpawnSpec {
            workspace_id: WorkspaceId(1),
            pane_id: PaneId(2),
            surface_id: SurfaceId(surface_id),
            cwd: "C:/work/alpha".to_string(),
        }
    }

    #[test]
    fn failed_entry_exposes_visible_status() {
        let entry = TerminalEntry::Failed {
            spec: spec(10),
            message: "pty refused startup".to_string(),
        };

        let state = state_from_entry(Some(&entry));

        assert_eq!(state.status, "terminal failed: pty refused startup");
        assert_eq!(state.snapshot, None);
        assert!(state.failed);
    }

    #[test]
    fn missing_entry_is_failed_state() {
        let state = state_from_entry(None);

        assert_eq!(state.status, "terminal missing");
        assert_eq!(state.snapshot, None);
        assert!(state.failed);
    }

    #[cfg(not(windows))]
    #[test]
    fn running_entry_without_health_is_unavailable() {
        let entry = TerminalEntry::Running { spec: spec(20) };

        let state = state_from_entry(Some(&entry));

        assert_eq!(state.status, "terminal unavailable");
        assert_eq!(state.snapshot, None);
        assert!(!state.failed);
    }

    #[test]
    fn health_status_includes_shell_and_size() {
        let health = TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24);

        assert_eq!(health_status(&health), "pwsh 80x24 running");
    }

    #[test]
    fn running_health_without_snapshot_is_not_failed() {
        let state = state_from_terminal_read(Some(TerminalEntrySnapshot {
            health: Some(TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24)),
            renderer_snapshot: None,
        }));

        assert_eq!(state.status, "pwsh 80x24 running");
        assert_eq!(state.snapshot, None);
        assert!(!state.failed);
    }

    #[test]
    fn exited_health_is_failed_and_visible() {
        let mut health = TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24);
        health.status = TerminalStatus::Exited;

        let state = state_from_terminal_read(Some(TerminalEntrySnapshot {
            health: Some(health),
            renderer_snapshot: None,
        }));

        assert_eq!(state.status, "pwsh 80x24 exited");
        assert_eq!(state.snapshot, None);
        assert!(state.failed);
    }

    #[test]
    fn failed_health_includes_last_error() {
        let mut health = TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24);
        health.status = TerminalStatus::Failed;
        health.last_error = Some("read failed".to_string());

        let state = state_from_terminal_read(Some(TerminalEntrySnapshot {
            health: Some(health),
            renderer_snapshot: None,
        }));

        assert_eq!(state.status, "terminal failed: read failed");
        assert_eq!(state.snapshot, None);
        assert!(state.failed);
    }

    #[test]
    fn read_model_preserves_renderer_snapshot() {
        let snapshot = TerminalRendererSnapshot {
            cols: 1,
            rows: 1,
            cells: Vec::new(),
            cursor: umux_terminal::TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection: None,
            scrollback_lines: 0,
            version: 7,
        };

        let state = state_from_terminal_read(Some(TerminalEntrySnapshot {
            health: Some(TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24)),
            renderer_snapshot: Some(snapshot.clone()),
        }));

        assert_eq!(state.status, "pwsh 80x24 running");
        assert_eq!(state.snapshot, Some(snapshot));
        assert!(!state.failed);
    }
}

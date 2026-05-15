// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TerminalStatus {
    Starting,
    Running,
    Exited,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum TerminalEvent {
    TitleChanged(String),
    Bell,
    Wakeup,
    ClipboardError(String),
    ChildExited(Option<i32>),
    Failed(String),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalHealth {
    pub shell: String,
    pub cwd: String,
    pub status: TerminalStatus,
    pub cols: u16,
    pub rows: u16,
    pub scrollback_lines: usize,
    pub bytes_read: u64,
    pub bytes_written: u64,
    pub last_error: Option<String>,
}

impl TerminalHealth {
    pub fn running(shell: impl Into<String>, cwd: impl Into<String>, cols: u16, rows: u16) -> Self {
        Self {
            shell: shell.into(),
            cwd: cwd.into(),
            status: TerminalStatus::Running,
            cols,
            rows,
            scrollback_lines: 0,
            bytes_read: 0,
            bytes_written: 0,
            last_error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_records_running_terminal() {
        let health = TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24);

        assert_eq!(health.status, TerminalStatus::Running);
        assert_eq!(health.shell, "pwsh");
        assert_eq!(health.cols, 80);
        assert_eq!(health.rows, 24);
        assert_eq!(health.last_error, None);
    }
}

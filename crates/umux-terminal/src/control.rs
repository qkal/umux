// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use thiserror::Error;
use umux_notify::TerminalNotification;

use crate::{
    PtyBackend, PtyError, TerminalHealth, TerminalInputRoute, TerminalInputRouter,
    TerminalKeyEvent, TerminalSession,
};

#[derive(Debug, Error, Eq, PartialEq)]
pub enum TerminalControlError {
    #[error("unknown terminal surface: {0}")]
    UnknownSurface(u64),
    #[error(transparent)]
    Pty(#[from] PtyError),
}

pub struct TerminalControlApi<B: PtyBackend> {
    sessions: HashMap<u64, TerminalSession<B>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalDebugInfo {
    pub surface_id: u64,
    pub health: TerminalHealth,
}

impl<B: PtyBackend> Default for TerminalControlApi<B> {
    fn default() -> Self {
        Self {
            sessions: HashMap::new(),
        }
    }
}

impl<B: PtyBackend> TerminalControlApi<B> {
    pub fn insert(&mut self, surface_id: u64, session: TerminalSession<B>) {
        self.sessions.insert(surface_id, session);
    }

    pub fn pump_once(
        &mut self,
        surface_id: u64,
    ) -> Result<Vec<TerminalNotification>, TerminalControlError> {
        Ok(self.session_mut(surface_id)?.pump_once()?)
    }

    pub fn read_screen(
        &self,
        surface_id: u64,
        include_scrollback: bool,
        line_limit: Option<usize>,
    ) -> Result<String, TerminalControlError> {
        let text = self.session(surface_id)?.screen_text(include_scrollback);
        let Some(line_limit) = line_limit else {
            return Ok(text);
        };

        Ok(text.lines().take(line_limit).collect::<Vec<_>>().join("\n"))
    }

    pub fn send_text(
        &mut self,
        surface_id: u64,
        text: impl AsRef<str>,
    ) -> Result<(), TerminalControlError> {
        self.session_mut(surface_id)?
            .write_input(text.as_ref().as_bytes())?;
        Ok(())
    }

    pub fn send_key(
        &mut self,
        surface_id: u64,
        event: TerminalKeyEvent,
    ) -> Result<(), TerminalControlError> {
        if let TerminalInputRoute::WriteBytes(bytes) = TerminalInputRouter::route_key(event) {
            self.session_mut(surface_id)?.write_input(bytes)?;
        } else {
            self.session_mut(surface_id)?;
        }
        Ok(())
    }

    pub fn clear_scrollback(&mut self, surface_id: u64) -> Result<(), TerminalControlError> {
        self.session_mut(surface_id)?.clear_scrollback();
        Ok(())
    }

    pub fn surface_health(&self, surface_id: u64) -> Result<TerminalHealth, TerminalControlError> {
        Ok(self.session(surface_id)?.health())
    }

    pub fn debug_terminals(&self) -> Vec<TerminalDebugInfo> {
        let mut terminals = self
            .sessions
            .iter()
            .map(|(surface_id, session)| TerminalDebugInfo {
                surface_id: *surface_id,
                health: session.health(),
            })
            .collect::<Vec<_>>();
        terminals.sort_by_key(|terminal| terminal.surface_id);
        terminals
    }

    fn session(&self, surface_id: u64) -> Result<&TerminalSession<B>, TerminalControlError> {
        self.sessions
            .get(&surface_id)
            .ok_or(TerminalControlError::UnknownSurface(surface_id))
    }

    fn session_mut(
        &mut self,
        surface_id: u64,
    ) -> Result<&mut TerminalSession<B>, TerminalControlError> {
        self.sessions
            .get_mut(&surface_id)
            .ok_or(TerminalControlError::UnknownSurface(surface_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakePtyBackend, ResolvedShell, TerminalSession, TerminalSessionConfig};

    #[test]
    fn control_api_reads_screen_and_health() {
        let mut api = TerminalControlApi::default();
        let session = TerminalSession::from_backend(
            TerminalSessionConfig::new(shell(), "C:/work/alpha", 80, 24),
            FakePtyBackend::new("ready\r\n"),
        );
        api.insert(4, session);
        api.pump_once(4).unwrap();

        assert!(api.read_screen(4, false, None).unwrap().contains("ready"));
        assert_eq!(api.surface_health(4).unwrap().shell, "pwsh");
    }

    #[test]
    fn public_surface_methods_report_unknown_surface() {
        let mut api: TerminalControlApi<FakePtyBackend> = TerminalControlApi::default();

        assert_eq!(
            api.pump_once(99),
            Err(TerminalControlError::UnknownSurface(99))
        );
        assert_eq!(
            api.read_screen(99, false, None),
            Err(TerminalControlError::UnknownSurface(99))
        );
        assert_eq!(
            api.send_text(99, "echo hi\r"),
            Err(TerminalControlError::UnknownSurface(99))
        );
        assert_eq!(
            api.clear_scrollback(99),
            Err(TerminalControlError::UnknownSurface(99))
        );
        assert_eq!(
            api.surface_health(99),
            Err(TerminalControlError::UnknownSurface(99))
        );
    }

    #[test]
    fn read_screen_line_limit_handles_zero_one_and_multiple_lines() {
        let api = api_with_output("one\r\ntwo\r\nthree\r\n");

        assert_eq!(api.read_screen(4, false, Some(0)).unwrap(), "");
        assert_eq!(api.read_screen(4, false, Some(1)).unwrap(), "one");
        assert_eq!(api.read_screen(4, false, Some(2)).unwrap(), "one\ntwo");
    }

    #[test]
    fn read_screen_honors_include_scrollback() {
        let api = api_with_output_with_size("one\r\ntwo\r\nthree", 5, 2);

        assert_eq!(api.read_screen(4, false, None).unwrap(), "two\nthree");
        assert_eq!(api.read_screen(4, true, None).unwrap(), "one\ntwo\nthree");
    }

    #[test]
    fn clear_scrollback_removes_history_from_control_reads() {
        let mut api = api_with_output_with_size("one\r\ntwo\r\nthree", 5, 2);

        api.clear_scrollback(4).unwrap();

        assert_eq!(api.read_screen(4, true, None).unwrap(), "two\nthree");
        assert_eq!(api.surface_health(4).unwrap().scrollback_lines, 0);
    }

    #[test]
    fn send_text_updates_health_bytes_written() {
        let mut api = api_with_output("");

        api.send_text(4, "echo hi\r").unwrap();

        assert_eq!(api.surface_health(4).unwrap().bytes_written, 8);
    }

    #[test]
    fn send_key_routes_terminal_input() {
        let mut api = api_with_output("");

        api.send_key(
            4,
            TerminalKeyEvent {
                key: crate::TerminalKey::Enter,
                ctrl: false,
                shift: false,
                alt: false,
                selection_present: false,
            },
        )
        .unwrap();

        assert_eq!(api.surface_health(4).unwrap().bytes_written, 1);
    }

    #[test]
    fn pump_once_returns_notifications_to_control_api() {
        let mut api = api_with_output_without_pump("\x1b]9;Build done\x07");

        let notifications = api.pump_once(4).unwrap();

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "Build done");
    }

    #[test]
    fn debug_terminals_lists_surface_health() {
        let api = api_with_output("");

        let terminals = api.debug_terminals();

        assert_eq!(terminals.len(), 1);
        assert_eq!(terminals[0].surface_id, 4);
        assert_eq!(terminals[0].health.shell, "pwsh");
    }

    fn api_with_output(output: impl AsRef<[u8]>) -> TerminalControlApi<FakePtyBackend> {
        api_with_output_with_size(output, 80, 24)
    }

    fn api_with_output_with_size(
        output: impl AsRef<[u8]>,
        cols: u16,
        rows: u16,
    ) -> TerminalControlApi<FakePtyBackend> {
        let mut api = api_with_output_without_pump_with_size(output, cols, rows);
        api.pump_once(4).unwrap();
        api
    }

    fn api_with_output_without_pump(
        output: impl AsRef<[u8]>,
    ) -> TerminalControlApi<FakePtyBackend> {
        api_with_output_without_pump_with_size(output, 80, 24)
    }

    fn api_with_output_without_pump_with_size(
        output: impl AsRef<[u8]>,
        cols: u16,
        rows: u16,
    ) -> TerminalControlApi<FakePtyBackend> {
        let mut api = TerminalControlApi::default();
        let session = TerminalSession::from_backend(
            TerminalSessionConfig::new(shell(), "C:/work/alpha", cols, rows),
            FakePtyBackend::new(output),
        );
        api.insert(4, session);
        api
    }

    fn shell() -> ResolvedShell {
        ResolvedShell {
            program: "pwsh".to_string(),
            args: Vec::new(),
            attempted: vec!["pwsh".to_string()],
            used_last_resort: false,
        }
    }
}

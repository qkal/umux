// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use thiserror::Error;

use crate::{PtyBackend, PtyError, TerminalHealth, TerminalSession};

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

    pub fn pump_once(&mut self, surface_id: u64) -> Result<(), TerminalControlError> {
        self.session_mut(surface_id)?.pump_once()?;
        Ok(())
    }

    pub fn read_screen(
        &self,
        surface_id: u64,
        _include_scrollback: bool,
        line_limit: Option<usize>,
    ) -> Result<String, TerminalControlError> {
        // `_include_scrollback` keeps the control API shape ready for future scrollback extraction.
        let text = self.session(surface_id)?.snapshot().visible_text();
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

    pub fn clear_scrollback(&mut self, surface_id: u64) -> Result<(), TerminalControlError> {
        self.session_mut(surface_id)?;
        Ok(())
    }

    pub fn surface_health(&self, surface_id: u64) -> Result<TerminalHealth, TerminalControlError> {
        Ok(self.session(surface_id)?.health())
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
    fn send_text_updates_health_bytes_written() {
        let mut api = api_with_output("");

        api.send_text(4, "echo hi\r").unwrap();

        assert_eq!(api.surface_health(4).unwrap().bytes_written, 8);
    }

    fn api_with_output(output: impl AsRef<[u8]>) -> TerminalControlApi<FakePtyBackend> {
        let mut api = TerminalControlApi::default();
        let session = TerminalSession::from_backend(
            TerminalSessionConfig::new(shell(), "C:/work/alpha", 80, 24),
            FakePtyBackend::new(output),
        );
        api.insert(4, session);
        api.pump_once(4).unwrap();
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

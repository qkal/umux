// SPDX-License-Identifier: GPL-3.0-or-later

use crate::{
    PtyBackend, PtyError, ResolvedShell, TerminalEmulator, TerminalHealth,
    TerminalRendererSnapshot, TerminalStatus,
};
use umux_notify::TerminalNotification;

const MAX_SESSION_DIMENSION: u16 = i16::MAX as u16;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TerminalSessionConfig {
    pub shell: ResolvedShell,
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
    pub scrollback_limit: usize,
}

impl TerminalSessionConfig {
    pub fn new(shell: ResolvedShell, cwd: impl Into<String>, cols: u16, rows: u16) -> Self {
        let (cols, rows) = clamp_session_size(cols, rows);

        Self {
            shell,
            cwd: cwd.into(),
            cols,
            rows,
            scrollback_limit: 10_000,
        }
    }
}

pub struct TerminalSession<B: PtyBackend> {
    backend: B,
    emulator: TerminalEmulator,
    health: TerminalHealth,
}

impl<B: PtyBackend> TerminalSession<B> {
    pub fn from_backend(config: TerminalSessionConfig, backend: B) -> Self {
        let emulator = TerminalEmulator::new(config.cols, config.rows, config.scrollback_limit);
        let health =
            TerminalHealth::running(config.shell.program, config.cwd, config.cols, config.rows);

        Self {
            backend,
            emulator,
            health,
        }
    }

    pub fn pump_once(&mut self) -> Result<Vec<TerminalNotification>, PtyError> {
        let output = self.backend.read_output().inspect_err(|error| {
            self.record_error(error);
        })?;
        if output.is_empty() {
            if self.poll_child_exit()?.is_some() {
                return self.drain_available_output();
            }
            return Ok(Vec::new());
        }

        let mut notifications = self.ingest_output(output);
        if self.poll_child_exit()?.is_some() {
            notifications.extend(self.drain_available_output()?);
        }

        Ok(notifications)
    }

    pub fn write_input(&mut self, input: impl AsRef<[u8]>) -> Result<(), PtyError> {
        let input = input.as_ref();
        self.backend.write_input(input).inspect_err(|error| {
            self.record_error(error);
        })?;
        self.health.bytes_written = self
            .health
            .bytes_written
            .saturating_add(u64::try_from(input.len()).unwrap_or(u64::MAX));

        Ok(())
    }

    pub fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let (cols, rows) = clamp_session_size(cols, rows);
        if self.health.cols == cols && self.health.rows == rows {
            return Ok(());
        }

        self.backend.resize(cols, rows).inspect_err(|error| {
            self.record_error(error);
        })?;
        self.emulator.resize(cols, rows);
        self.health.cols = cols;
        self.health.rows = rows;

        Ok(())
    }

    pub fn snapshot(&self) -> TerminalRendererSnapshot {
        self.emulator.snapshot()
    }

    pub fn screen_text(&self, include_scrollback: bool) -> String {
        self.emulator.screen_text(include_scrollback)
    }

    pub fn clear_scrollback(&mut self) {
        self.emulator.clear_scrollback();
        self.health.scrollback_lines = 0;
    }

    pub fn health(&self) -> TerminalHealth {
        self.health.clone()
    }

    pub fn poll_child_exit(&mut self) -> Result<Option<i32>, PtyError> {
        let exit = self.backend.child_exited().inspect_err(|error| {
            self.record_error(error);
        })?;
        if exit.is_some() {
            self.health.status = TerminalStatus::Exited;
        }

        Ok(exit)
    }

    fn record_error(&mut self, error: &PtyError) {
        self.health.last_error = Some(error.to_string());
        self.health.status = TerminalStatus::Failed;
    }

    fn drain_available_output(&mut self) -> Result<Vec<TerminalNotification>, PtyError> {
        let mut notifications = Vec::new();
        loop {
            let output = self.backend.read_output().inspect_err(|error| {
                self.record_error(error);
            })?;
            if output.is_empty() {
                return Ok(notifications);
            }

            notifications.extend(self.ingest_output(output));
        }
    }

    fn ingest_output(&mut self, output: Vec<u8>) -> Vec<TerminalNotification> {
        self.health.bytes_read = self
            .health
            .bytes_read
            .saturating_add(u64::try_from(output.len()).unwrap_or(u64::MAX));
        let notifications = self.emulator.feed_bytes(&output);
        self.health.scrollback_lines = self.emulator.snapshot().scrollback_lines as usize;
        notifications
    }
}

fn clamp_session_size(cols: u16, rows: u16) -> (u16, u16) {
    (
        cols.clamp(1, MAX_SESSION_DIMENSION),
        rows.clamp(1, MAX_SESSION_DIMENSION),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakePtyBackend, ResolvedShell};
    use std::collections::VecDeque;

    #[test]
    fn session_ingests_output_and_updates_snapshot() {
        let backend = FakePtyBackend::new("hello\r\n");
        let shell = ResolvedShell {
            program: "pwsh".to_string(),
            args: Vec::new(),
            attempted: vec!["pwsh".to_string()],
            used_last_resort: false,
        };
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(shell, "C:/work/alpha", 80, 24),
            backend,
        );

        session.pump_once().unwrap();

        assert!(session.snapshot().visible_text().contains("hello"));
        assert_eq!(session.health().bytes_read, 7);
    }

    #[test]
    fn session_pump_returns_terminal_notifications() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            FakePtyBackend::new("\x1b]9;Build done\x07"),
        );

        let notifications = session.pump_once().unwrap();

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "Build done");
    }

    #[test]
    fn session_screen_text_can_include_scrollback() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 5, 2),
            FakePtyBackend::new("one\r\ntwo\r\nthree"),
        );
        session.pump_once().unwrap();

        assert_eq!(session.screen_text(false), "two\nthree");
        assert_eq!(session.screen_text(true), "one\ntwo\nthree");
    }

    #[test]
    fn session_clear_scrollback_preserves_visible_screen() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 5, 2),
            FakePtyBackend::new("one\r\ntwo\r\nthree"),
        );
        session.pump_once().unwrap();

        session.clear_scrollback();

        assert_eq!(session.screen_text(true), "two\nthree");
        assert_eq!(session.health().scrollback_lines, 0);
    }

    #[test]
    fn session_poll_child_exit_marks_health_exited() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new("").with_child_exit(0),
        );

        let exit = session.poll_child_exit().unwrap();

        assert_eq!(exit, Some(0));
        assert_eq!(session.health().status, crate::TerminalStatus::Exited);
    }

    #[test]
    fn session_drains_buffered_output_after_child_exit_is_observed() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            ChunkedExitPtyBackend::new([
                b"first\r\n".to_vec(),
                b"second\r\n\x1b]9;done\x07".to_vec(),
            ]),
        );

        let notifications = session.pump_once().unwrap();

        assert_eq!(session.health().status, crate::TerminalStatus::Exited);
        assert_eq!(session.health().bytes_read, 24);
        assert!(session.snapshot().visible_text().contains("first"));
        assert!(session.snapshot().visible_text().contains("second"));
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "done");
    }

    #[test]
    fn session_drains_buffered_output_when_exit_follows_empty_read() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            ChunkedExitPtyBackend::new([Vec::new(), b"late\r\n\x1b]9;late notice\x07".to_vec()]),
        );

        let notifications = session.pump_once().unwrap();

        assert_eq!(session.health().status, crate::TerminalStatus::Exited);
        assert!(session.snapshot().visible_text().contains("late"));
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].message, "late notice");
    }

    #[test]
    fn session_resize_updates_backend_and_health() {
        let backend = FakePtyBackend::new("");
        let shell = ResolvedShell {
            program: "pwsh".to_string(),
            args: Vec::new(),
            attempted: vec!["pwsh".to_string()],
            used_last_resort: false,
        };
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(shell, "C:/work/alpha", 80, 24),
            backend,
        );

        session.resize(100, 40).unwrap();

        assert_eq!(session.health().cols, 100);
        assert_eq!(session.health().rows, 40);
    }

    #[test]
    fn session_resize_clamps_oversized_dimensions_to_backend_limit() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new(""),
        );

        session.resize(u16::MAX, 1).unwrap();

        assert_eq!(session.health().cols, i16::MAX as u16);
        assert_eq!(session.health().rows, 1);
        assert_eq!(session.snapshot().cols, i16::MAX as u16);
        assert_eq!(session.snapshot().rows, 1);
        assert_eq!(session.backend.last_resize, Some((i16::MAX as u16, 1)));
    }

    #[test]
    fn session_write_input_records_bytes_written_on_success() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new(""),
        );

        session.write_input(b"echo hi\r").unwrap();

        assert_eq!(session.health().bytes_written, 8);
        assert_eq!(session.backend.written_input, b"echo hi\r");
    }

    #[test]
    fn session_backend_error_sets_last_error_without_incrementing_counters() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new("").with_write_error("write failed"),
        );

        let error = session.write_input(b"echo hi\r").unwrap_err();

        assert_eq!(error, PtyError::Io("write failed".to_string()));
        assert_eq!(
            session.health().last_error,
            Some("pty I/O error: write failed".to_string())
        );
        assert_eq!(session.health().status, crate::TerminalStatus::Failed);
        assert_eq!(session.health().bytes_read, 0);
        assert_eq!(session.health().bytes_written, 0);
    }

    #[test]
    fn session_backend_read_error_sets_last_error_without_incrementing_counters() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new("").with_read_error("read failed"),
        );

        let error = session.pump_once().unwrap_err();

        assert_eq!(error, PtyError::Io("read failed".to_string()));
        assert_eq!(
            session.health().last_error,
            Some("pty I/O error: read failed".to_string())
        );
        assert_eq!(session.health().status, crate::TerminalStatus::Failed);
        assert_eq!(session.health().bytes_read, 0);
        assert_eq!(session.health().bytes_written, 0);
    }

    #[test]
    fn session_backend_resize_error_sets_last_error_without_updating_health() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new("").with_resize_error("resize failed"),
        );

        let error = session.resize(100, 40).unwrap_err();

        assert_eq!(error, PtyError::Io("resize failed".to_string()));
        assert_eq!(
            session.health().last_error,
            Some("pty I/O error: resize failed".to_string())
        );
        assert_eq!(session.health().status, crate::TerminalStatus::Failed);
        assert_eq!(session.health().cols, 80);
        assert_eq!(session.health().rows, 24);
    }

    #[test]
    fn session_resize_noop_does_not_call_backend() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(test_shell(), "C:/work/alpha", 80, 24),
            RecordingPtyBackend::new(""),
        );

        session.resize(80, 24).unwrap();

        assert_eq!(session.backend.resize_calls, 0);
        assert_eq!(session.backend.last_resize, None);
    }

    fn test_shell() -> ResolvedShell {
        ResolvedShell {
            program: "pwsh".to_string(),
            args: Vec::new(),
            attempted: vec!["pwsh".to_string()],
            used_last_resort: false,
        }
    }

    #[derive(Debug)]
    struct RecordingPtyBackend {
        output: Vec<u8>,
        written_input: Vec<u8>,
        resize_calls: usize,
        last_resize: Option<(u16, u16)>,
        read_error: Option<String>,
        write_error: Option<String>,
        resize_error: Option<String>,
        child_exit: Option<i32>,
    }

    impl RecordingPtyBackend {
        fn new(output: impl AsRef<[u8]>) -> Self {
            Self {
                output: output.as_ref().to_vec(),
                written_input: Vec::new(),
                resize_calls: 0,
                last_resize: None,
                read_error: None,
                write_error: None,
                resize_error: None,
                child_exit: None,
            }
        }

        fn with_read_error(mut self, message: impl Into<String>) -> Self {
            self.read_error = Some(message.into());
            self
        }

        fn with_write_error(mut self, message: impl Into<String>) -> Self {
            self.write_error = Some(message.into());
            self
        }

        fn with_resize_error(mut self, message: impl Into<String>) -> Self {
            self.resize_error = Some(message.into());
            self
        }

        fn with_child_exit(mut self, status: i32) -> Self {
            self.child_exit = Some(status);
            self
        }
    }

    impl PtyBackend for RecordingPtyBackend {
        fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
            if let Some(message) = &self.read_error {
                return Err(PtyError::Io(message.clone()));
            }

            Ok(std::mem::take(&mut self.output))
        }

        fn write_input(&mut self, input: &[u8]) -> Result<(), PtyError> {
            if let Some(message) = &self.write_error {
                return Err(PtyError::Io(message.clone()));
            }

            self.written_input.extend_from_slice(input);
            Ok(())
        }

        fn resize(&mut self, cols: u16, rows: u16) -> Result<(), PtyError> {
            self.resize_calls += 1;
            if let Some(message) = &self.resize_error {
                return Err(PtyError::Io(message.clone()));
            }

            self.last_resize = Some((cols, rows));
            Ok(())
        }

        fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
            Ok(self.child_exit.take())
        }
    }

    #[derive(Debug)]
    struct ChunkedExitPtyBackend {
        output: VecDeque<Vec<u8>>,
        child_exit_calls: usize,
    }

    impl ChunkedExitPtyBackend {
        fn new(chunks: impl IntoIterator<Item = Vec<u8>>) -> Self {
            Self {
                output: chunks.into_iter().collect(),
                child_exit_calls: 0,
            }
        }
    }

    impl PtyBackend for ChunkedExitPtyBackend {
        fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
            Ok(self.output.pop_front().unwrap_or_default())
        }

        fn write_input(&mut self, _input: &[u8]) -> Result<(), PtyError> {
            Ok(())
        }

        fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), PtyError> {
            Ok(())
        }

        fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
            self.child_exit_calls += 1;
            if self.child_exit_calls == 1 {
                Ok(Some(0))
            } else {
                Ok(None)
            }
        }
    }
}

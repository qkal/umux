// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(windows)]
use std::borrow::Cow;
#[cfg(windows)]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(windows)]
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use std::thread::JoinHandle;

#[cfg(windows)]
use alacritty_terminal::event::WindowSize;
#[cfg(windows)]
use alacritty_terminal::event_loop::{EventLoop, EventLoopSender, Msg, State};
#[cfg(windows)]
use alacritty_terminal::grid::Dimensions;
#[cfg(windows)]
use alacritty_terminal::sync::FairMutex;
#[cfg(windows)]
use alacritty_terminal::term::{Config, Term};

#[cfg(windows)]
use crate::emulator::{TerminalDimensions, snapshot_from_term};
#[cfg(windows)]
use crate::pty::clamp_pty_size;
#[cfg(windows)]
use crate::{
    AlacrittyPtyBackend, PtyError, PtySpawnConfig, TerminalEventSink, TerminalHealth,
    TerminalPalette, TerminalRendererSnapshot, TerminalStatus,
};

#[cfg(windows)]
type LiveEventLoop = EventLoop<alacritty_terminal::tty::Pty, TerminalEventSink>;
#[cfg(windows)]
type LiveThread = JoinHandle<(LiveEventLoop, State)>;

#[cfg(windows)]
pub struct LiveTerminalSession {
    terminal: Arc<FairMutex<Term<TerminalEventSink>>>,
    palette: TerminalPalette,
    sender: EventLoopSender,
    thread: Mutex<Option<LiveThread>>,
    shell: String,
    cwd: String,
    size: Mutex<(u16, u16)>,
    version: AtomicU64,
    bytes_read: AtomicU64,
    bytes_written: AtomicU64,
    last_visible_text_len: AtomicU64,
    last_error: Mutex<Option<String>>,
}

#[cfg(windows)]
impl LiveTerminalSession {
    pub fn spawn(config: PtySpawnConfig) -> Result<Self, PtyError> {
        let (cols, rows) = clamp_pty_size(config.cols, config.rows);
        let dimensions = TerminalDimensions::new(cols, rows);
        let sink = TerminalEventSink::default();
        let term_config = Config {
            scrolling_history: 10_000,
            ..Default::default()
        };
        let terminal = Arc::new(FairMutex::new(Term::new(
            term_config,
            &dimensions,
            sink.clone(),
        )));
        let pty = AlacrittyPtyBackend::spawn_raw(config.clone())?;
        let event_loop =
            EventLoop::new(terminal.clone(), sink, pty, false, false).map_err(io_error)?;
        let sender = event_loop.channel();
        let thread = event_loop.spawn();

        Ok(Self {
            terminal,
            palette: TerminalPalette::default(),
            sender,
            thread: Mutex::new(Some(thread)),
            shell: config.shell.program,
            cwd: config.cwd,
            size: Mutex::new((cols, rows)),
            version: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            last_visible_text_len: AtomicU64::new(0),
            last_error: Mutex::new(None),
        })
    }

    pub fn send_input(&self, input: impl AsRef<[u8]>) -> Result<(), PtyError> {
        let input = input.as_ref();
        self.sender
            .send(Msg::Input(Cow::Owned(input.to_vec())))
            .map_err(|error| self.record_error(error))?;
        self.bytes_written
            .fetch_add(input.len() as u64, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        let window_size = window_size(cols, rows);
        self.sender
            .send(Msg::Resize(window_size))
            .map_err(|error| self.record_error(error))?;
        apply_resize_state(&self.terminal, &self.size, &self.version, cols, rows);
        Ok(())
    }

    pub fn snapshot(&self) -> TerminalRendererSnapshot {
        let term = self.terminal.lock();
        let version = self.version.fetch_add(1, Ordering::Relaxed);
        let snapshot = snapshot_from_term(&term, &self.palette, version);
        update_visible_text_estimate(
            &self.last_visible_text_len,
            &self.bytes_read,
            snapshot.visible_text().len() as u64,
        );
        snapshot
    }

    pub fn health(&self) -> TerminalHealth {
        let (cols, rows) = *self.size.lock().expect("terminal size lock poisoned");
        let status = if self.is_alive() {
            TerminalStatus::Running
        } else {
            TerminalStatus::Exited
        };
        let scrollback_lines = self.terminal.lock().grid().history_size();

        TerminalHealth {
            shell: self.shell.clone(),
            cwd: self.cwd.clone(),
            status,
            cols,
            rows,
            scrollback_lines,
            bytes_read: self.bytes_read.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
            last_error: self
                .last_error
                .lock()
                .expect("terminal error lock poisoned")
                .clone(),
        }
    }

    pub fn is_alive(&self) -> bool {
        self.thread
            .lock()
            .expect("terminal thread lock poisoned")
            .as_ref()
            .is_some_and(|thread| !thread.is_finished())
    }

    fn record_error(&self, error: impl std::fmt::Display) -> PtyError {
        let message = error.to_string();
        *self
            .last_error
            .lock()
            .expect("terminal error lock poisoned") = Some(message.clone());
        PtyError::Io(message)
    }
}

#[cfg(windows)]
impl Drop for LiveTerminalSession {
    fn drop(&mut self) {
        let _ = self.sender.send(Msg::Shutdown);
        if let Some(thread) = self
            .thread
            .lock()
            .expect("terminal thread lock poisoned")
            .take()
        {
            let _ = std::thread::Builder::new()
                .name("PTY shutdown join".to_string())
                .spawn(move || {
                    let _ = thread.join();
                });
        }
    }
}

#[cfg(windows)]
fn apply_resize_state(
    terminal: &Arc<FairMutex<Term<TerminalEventSink>>>,
    size: &Mutex<(u16, u16)>,
    version: &AtomicU64,
    cols: u16,
    rows: u16,
) -> WindowSize {
    let window_size = window_size(cols, rows);
    terminal.lock().resize(TerminalDimensions::new(
        window_size.num_cols,
        window_size.num_lines,
    ));
    *size.lock().expect("terminal size lock poisoned") =
        (window_size.num_cols, window_size.num_lines);
    version.fetch_add(1, Ordering::Relaxed);
    window_size
}

#[cfg(windows)]
fn update_visible_text_estimate(
    last_visible_text_len: &AtomicU64,
    bytes_read: &AtomicU64,
    current_len: u64,
) {
    // Alacritty's event loop owns PTY reads, so live health reports visible-text growth.
    let previous = last_visible_text_len.swap(current_len, Ordering::Relaxed);
    if current_len > previous {
        bytes_read.fetch_add(current_len - previous, Ordering::Relaxed);
    }
}

#[cfg(windows)]
fn window_size(cols: u16, rows: u16) -> WindowSize {
    let (cols, rows) = clamp_pty_size(cols, rows);

    WindowSize {
        num_lines: rows,
        num_cols: cols,
        cell_width: 0,
        cell_height: 0,
    }
}

#[cfg(windows)]
fn io_error(error: std::io::Error) -> PtyError {
    PtyError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
    #[test]
    fn window_size_clamps_to_conpty_safe_dimensions() {
        let size = window_size(0, u16::MAX);

        assert_eq!(size.num_cols, 1);
        assert_eq!(size.num_lines, i16::MAX as u16);
    }

    #[cfg(windows)]
    #[test]
    fn resize_state_updates_terminal_snapshot_dimensions() {
        let dimensions = TerminalDimensions::new(80, 24);
        let sink = TerminalEventSink::default();
        let terminal = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &dimensions,
            sink,
        )));
        let size = Mutex::new((80, 24));
        let version = AtomicU64::new(0);

        apply_resize_state(&terminal, &size, &version, u16::MAX, 10);

        let snapshot = snapshot_from_term(&terminal.lock(), &TerminalPalette::default(), 0);
        assert_eq!(snapshot.cols, i16::MAX as u16);
        assert_eq!(snapshot.rows, 10);
        assert_eq!(*size.lock().unwrap(), (i16::MAX as u16, 10));
    }

    #[cfg(windows)]
    #[test]
    fn visible_text_estimate_counts_growth_only() {
        let previous = AtomicU64::new(0);
        let total = AtomicU64::new(0);

        update_visible_text_estimate(&previous, &total, 5);
        update_visible_text_estimate(&previous, &total, 3);
        update_visible_text_estimate(&previous, &total, 8);

        assert_eq!(total.load(Ordering::Relaxed), 10);
    }
}

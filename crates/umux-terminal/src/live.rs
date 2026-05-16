// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(windows)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::sync::{Arc, Mutex};
#[cfg(windows)]
use std::thread::JoinHandle;
#[cfg(windows)]
use std::time::Duration;

#[cfg(windows)]
use crossbeam_channel::{Receiver, Sender};
#[cfg(windows)]
use umux_notify::TerminalNotification;

#[cfg(windows)]
use crate::pty::clamp_pty_size;
#[cfg(windows)]
use crate::{
    AlacrittyPtyBackend, PtyBackend, PtyError, PtySpawnConfig, TerminalHealth,
    TerminalRendererSnapshot, TerminalSession, TerminalSessionConfig, TerminalStatus,
};

#[cfg(windows)]
pub struct LiveTerminalSession {
    tx: Sender<LiveCommand>,
    state: Arc<Mutex<LiveState>>,
    alive: Arc<AtomicBool>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

#[cfg(windows)]
#[derive(Debug)]
enum LiveCommand {
    Input(Vec<u8>),
    Resize(u16, u16),
    Shutdown,
}

#[cfg(windows)]
struct LiveState {
    snapshot: TerminalRendererSnapshot,
    health: TerminalHealth,
    notifications: Vec<TerminalNotification>,
}

#[cfg(windows)]
impl LiveTerminalSession {
    pub fn spawn(config: PtySpawnConfig) -> Result<Self, PtyError> {
        let (cols, rows) = clamp_pty_size(config.cols, config.rows);
        let backend = AlacrittyPtyBackend::spawn(config.clone())?;
        let session_config =
            TerminalSessionConfig::new(config.shell.clone(), config.cwd.clone(), cols, rows);
        let mut session = TerminalSession::from_backend(session_config, backend);
        let state = Arc::new(Mutex::new(LiveState {
            snapshot: session.snapshot(),
            health: session.health(),
            notifications: Vec::new(),
        }));
        let alive = Arc::new(AtomicBool::new(true));
        let (tx, rx) = crossbeam_channel::unbounded();
        let thread_state = state.clone();
        let thread_alive = alive.clone();
        let thread = std::thread::Builder::new()
            .name("umux-terminal-live".to_string())
            .spawn(move || run_live_pump(&mut session, rx, thread_state, thread_alive))
            .map_err(|error| PtyError::Io(error.to_string()))?;

        Ok(Self {
            tx,
            state,
            alive,
            thread: Mutex::new(Some(thread)),
        })
    }

    pub fn send_input(&self, input: impl AsRef<[u8]>) -> Result<(), PtyError> {
        self.tx
            .send(LiveCommand::Input(input.as_ref().to_vec()))
            .map_err(channel_error)
    }

    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), PtyError> {
        self.tx
            .send(LiveCommand::Resize(cols, rows))
            .map_err(channel_error)
    }

    pub fn snapshot(&self) -> TerminalRendererSnapshot {
        self.state
            .lock()
            .expect("terminal state lock poisoned")
            .snapshot
            .clone()
    }

    pub fn health(&self) -> TerminalHealth {
        self.state
            .lock()
            .expect("terminal state lock poisoned")
            .health
            .clone()
    }

    pub fn drain_notifications(&self) -> Vec<TerminalNotification> {
        std::mem::take(
            &mut self
                .state
                .lock()
                .expect("terminal state lock poisoned")
                .notifications,
        )
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::Relaxed)
    }
}

#[cfg(windows)]
impl Drop for LiveTerminalSession {
    fn drop(&mut self) {
        let _ = self.tx.send(LiveCommand::Shutdown);
        self.alive.store(false, Ordering::Relaxed);
        if let Some(thread) = self
            .thread
            .lock()
            .expect("terminal thread lock poisoned")
            .take()
        {
            let _ = std::thread::Builder::new()
                .name("umux-terminal-live-join".to_string())
                .spawn(move || {
                    let _ = thread.join();
                });
        }
    }
}

#[cfg(windows)]
fn run_live_pump<B: PtyBackend>(
    session: &mut TerminalSession<B>,
    rx: Receiver<LiveCommand>,
    state: Arc<Mutex<LiveState>>,
    alive: Arc<AtomicBool>,
) {
    while alive.load(Ordering::Relaxed) {
        match rx.recv_timeout(Duration::from_millis(10)) {
            Ok(LiveCommand::Input(input)) => {
                let _ = session.write_input(input);
            }
            Ok(LiveCommand::Resize(cols, rows)) => {
                let _ = session.resize(cols, rows);
            }
            Ok(LiveCommand::Shutdown) => break,
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => break,
        }

        match session.pump_once() {
            Ok(notifications) => update_live_state(session, &state, notifications),
            Err(_) => update_live_state(session, &state, Vec::new()),
        }

        if matches!(
            session.health().status,
            TerminalStatus::Exited | TerminalStatus::Failed
        ) {
            break;
        }
    }

    alive.store(false, Ordering::Relaxed);
    update_live_state(session, &state, Vec::new());
}

#[cfg(windows)]
fn update_live_state<B: PtyBackend>(
    session: &TerminalSession<B>,
    state: &Arc<Mutex<LiveState>>,
    notifications: Vec<TerminalNotification>,
) {
    let mut state = state.lock().expect("terminal state lock poisoned");
    state.snapshot = session.snapshot();
    state.health = session.health();
    state.notifications.extend(notifications);
}

#[cfg(windows)]
fn channel_error(error: impl std::fmt::Display) -> PtyError {
    PtyError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::*;

    #[cfg(windows)]
    #[test]
    fn live_state_drains_notifications_once() {
        let mut emulator = crate::TerminalEmulator::new(5, 2, 100);
        let notification = emulator.feed_bytes(b"\x1b]9;ready\x07").remove(0);
        let state = Arc::new(Mutex::new(LiveState {
            snapshot: emulator.snapshot(),
            health: TerminalHealth::running("pwsh", "C:/work/alpha", 5, 2),
            notifications: vec![notification.clone()],
        }));
        let (tx, _rx) = crossbeam_channel::unbounded();
        let session = LiveTerminalSession {
            tx,
            state,
            alive: Arc::new(AtomicBool::new(true)),
            thread: Mutex::new(None),
        };

        assert_eq!(session.drain_notifications(), vec![notification]);
        assert!(session.drain_notifications().is_empty());
    }

    #[cfg(windows)]
    #[test]
    fn live_pump_stops_and_marks_state_exited_when_child_exits() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(
                crate::ResolvedShell {
                    program: "pwsh".to_string(),
                    args: Vec::new(),
                    attempted: vec!["pwsh".to_string()],
                    used_last_resort: false,
                },
                "C:/work/alpha",
                5,
                2,
            ),
            ExitPtyBackend,
        );
        let state = Arc::new(Mutex::new(LiveState {
            snapshot: session.snapshot(),
            health: session.health(),
            notifications: Vec::new(),
        }));
        let alive = Arc::new(AtomicBool::new(true));
        let (_tx, rx) = crossbeam_channel::unbounded();

        run_live_pump(&mut session, rx, state.clone(), alive.clone());

        assert!(!alive.load(Ordering::Relaxed));
        assert_eq!(
            state.lock().unwrap().health.status,
            crate::TerminalStatus::Exited
        );
    }

    #[cfg(windows)]
    #[test]
    fn live_pump_stops_and_marks_state_failed_when_backend_errors() {
        let mut session = TerminalSession::from_backend(
            TerminalSessionConfig::new(
                crate::ResolvedShell {
                    program: "pwsh".to_string(),
                    args: Vec::new(),
                    attempted: vec!["pwsh".to_string()],
                    used_last_resort: false,
                },
                "C:/work/alpha",
                5,
                2,
            ),
            ErrorPtyBackend,
        );
        let state = Arc::new(Mutex::new(LiveState {
            snapshot: session.snapshot(),
            health: session.health(),
            notifications: Vec::new(),
        }));
        let alive = Arc::new(AtomicBool::new(true));
        let (_tx, rx) = crossbeam_channel::unbounded();

        run_live_pump(&mut session, rx, state.clone(), alive.clone());

        assert!(!alive.load(Ordering::Relaxed));
        assert_eq!(
            state.lock().unwrap().health.status,
            crate::TerminalStatus::Failed
        );
    }

    #[cfg(windows)]
    struct ExitPtyBackend;

    #[cfg(windows)]
    impl crate::PtyBackend for ExitPtyBackend {
        fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
            Ok(Vec::new())
        }

        fn write_input(&mut self, _input: &[u8]) -> Result<(), PtyError> {
            Ok(())
        }

        fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), PtyError> {
            Ok(())
        }

        fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
            Ok(Some(0))
        }
    }

    #[cfg(windows)]
    struct ErrorPtyBackend;

    #[cfg(windows)]
    impl crate::PtyBackend for ErrorPtyBackend {
        fn read_output(&mut self) -> Result<Vec<u8>, PtyError> {
            Err(PtyError::Io("read failed".to_string()))
        }

        fn write_input(&mut self, _input: &[u8]) -> Result<(), PtyError> {
            Ok(())
        }

        fn resize(&mut self, _cols: u16, _rows: u16) -> Result<(), PtyError> {
            Ok(())
        }

        fn child_exited(&mut self) -> Result<Option<i32>, PtyError> {
            Ok(None)
        }
    }
}

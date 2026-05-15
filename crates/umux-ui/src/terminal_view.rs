// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(windows)]
use std::time::Duration;

use floem::event::{Event, EventListener};
use floem::ext_event::create_signal_from_channel;
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::prelude::*;
#[cfg(windows)]
use umux_terminal::{
    LiveTerminalSession, PtySpawnConfig, ShellResolver, StartupEnvironment, TerminalHealth,
};
use umux_terminal::{
    TerminalInputRoute, TerminalInputRouter, TerminalKey, TerminalKeyEvent, TerminalMetrics,
};

#[cfg(windows)]
type UiTerminalSession = LiveTerminalSession;

#[cfg(not(windows))]
struct UiTerminalSession;

struct TerminalSessionController {
    session: Arc<UiTerminalSession>,
    refresh_stop: Arc<AtomicBool>,
}

impl TerminalSessionController {
    fn send_input(&self, input: impl AsRef<[u8]>) {
        let _ = self.session.send_input(input);
    }

    fn resize(&self, cols: u16, rows: u16) {
        let _ = self.session.resize(cols, rows);
    }
}

impl Drop for TerminalSessionController {
    fn drop(&mut self) {
        self.refresh_stop.store(true, Ordering::Relaxed);
    }
}

type TerminalSessionHandle = Arc<TerminalSessionController>;

const TERMINAL_BG: Color = Color::rgb8(0x11, 0x13, 0x16);
const TERMINAL_TEXT: Color = Color::rgb8(0xe7, 0xea, 0xf0);
const TERMINAL_MUTED: Color = Color::rgb8(0x9b, 0xa3, 0xaf);

pub fn terminal_status_line(shell: &str, cols: u16, rows: u16) -> String {
    format!("{shell} {cols}x{rows}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalUiState {
    status: String,
    text: String,
}

impl TerminalUiState {
    fn initial(shell: &str, text: &str) -> Self {
        Self {
            status: terminal_status_line(shell, 80, 24),
            text: text.to_string(),
        }
    }

    #[cfg(windows)]
    fn from_health(health: TerminalHealth, text: String) -> Self {
        Self {
            status: terminal_status_line(&health.shell, health.cols, health.rows),
            text: nonblank_terminal_text(text),
        }
    }
}

pub fn terminal_view() -> impl IntoView {
    terminal_view_for_cwd(".".to_string())
}

pub fn terminal_view_for_cwd(cwd: String) -> impl IntoView {
    let initial = TerminalUiState::initial("pwsh", "umux terminal MVP");
    let initial_status = initial.status.clone();
    let initial_text = initial.text.clone();
    let (state_tx, state_rx) = crossbeam_channel::unbounded::<TerminalUiState>();
    let state = create_signal_from_channel(state_rx);
    let _ = state_tx.send(initial.clone());

    let session = start_terminal_session(cwd, state_tx);
    let session_for_key = session.clone();
    let session_for_resize = session;

    v_stack((
        label(move || {
            state
                .get()
                .map(|state| state.status)
                .unwrap_or_else(|| initial_status.clone())
        })
        .style(|s| s.color(TERMINAL_MUTED).font_size(12.0)),
        label(move || {
            state
                .get()
                .map(|state| state.text)
                .unwrap_or_else(|| initial_text.clone())
        })
        .style(|s| s.color(TERMINAL_TEXT).font_size(13.0)),
    ))
    .keyboard_navigable()
    .on_event_stop(EventListener::KeyDown, move |event| {
        let Event::KeyDown(event) = event else {
            return;
        };
        let Some(session) = &session_for_key else {
            return;
        };

        if let TerminalInputRoute::WriteBytes(bytes) = route_key_event(event) {
            session.send_input(bytes);
        }
    })
    .on_resize(move |rect| {
        let Some(session) = &session_for_resize else {
            return;
        };
        let size =
            TerminalMetrics::new(8.0, 16.0).cols_rows(rect.width() as f32, rect.height() as f32);
        session.resize(size.cols, size.rows);
    })
    .style(|s| {
        s.width_full()
            .height_full()
            .padding(12.0)
            .gap(8.0)
            .background(TERMINAL_BG)
            .font_family("Cascadia Mono".to_string())
    })
}

#[cfg(windows)]
fn start_terminal_session(
    cwd: String,
    state_tx: crossbeam_channel::Sender<TerminalUiState>,
) -> Option<TerminalSessionHandle> {
    let shell = ShellResolver::from_path().resolve();
    let env = StartupEnvironment::new(1, 1, 1, cwd.clone()).into_pairs();
    let config = PtySpawnConfig {
        shell,
        cwd,
        env,
        cols: 80,
        rows: 24,
    };

    let session = match LiveTerminalSession::spawn(config) {
        Ok(session) => session,
        Err(error) => {
            let _ = state_tx.send(TerminalUiState {
                status: "terminal failed".to_string(),
                text: format!("Unable to start terminal: {error}"),
            });
            return None;
        }
    };

    let _ = session.send_input(b"Write-Output umux-ready\r");
    let controller = Arc::new(TerminalSessionController {
        session: Arc::new(session),
        refresh_stop: Arc::new(AtomicBool::new(false)),
    });
    let refresh_stop = controller.refresh_stop.clone();
    let refresh_session = Arc::downgrade(&controller);
    std::thread::spawn(move || {
        while refresh_loop_should_continue(&refresh_stop) {
            let Some(controller) = refresh_session.upgrade() else {
                return;
            };
            if !controller.session.is_alive() {
                break;
            }

            let snapshot = controller.session.snapshot();
            let health = controller.session.health();
            if state_tx
                .send(TerminalUiState::from_health(
                    health,
                    snapshot.visible_text(),
                ))
                .is_err()
            {
                return;
            }
            std::thread::sleep(Duration::from_millis(33));
        }

        if let Some(controller) = refresh_session.upgrade() {
            let snapshot = controller.session.snapshot();
            let health = controller.session.health();
            let _ = state_tx.send(TerminalUiState::from_health(
                health,
                snapshot.visible_text(),
            ));
        }
    });

    Some(controller)
}

#[cfg(not(windows))]
fn start_terminal_session(
    _cwd: String,
    state_tx: crossbeam_channel::Sender<TerminalUiState>,
) -> Option<TerminalSessionHandle> {
    let _ = state_tx.send(TerminalUiState::initial(
        "shell",
        "umux terminal MVP\nlive terminal is available on Windows",
    ));
    Some(Arc::new(TerminalSessionController {
        session: Arc::new(UiTerminalSession),
        refresh_stop: Arc::new(AtomicBool::new(false)),
    }))
}

#[cfg(not(windows))]
impl UiTerminalSession {
    fn send_input(&self, _input: impl AsRef<[u8]>) -> Result<(), ()> {
        Ok(())
    }

    fn resize(&self, _cols: u16, _rows: u16) -> Result<(), ()> {
        Ok(())
    }
}

fn route_key_event(event: &floem::keyboard::KeyEvent) -> TerminalInputRoute {
    let Some(key) = map_key(&event.key.logical_key) else {
        return TerminalInputRoute::Ignore;
    };

    TerminalInputRouter::route_key(TerminalKeyEvent {
        key,
        ctrl: event.modifiers.contains(Modifiers::CONTROL),
        shift: event.modifiers.contains(Modifiers::SHIFT),
        alt: event.modifiers.contains(Modifiers::ALT),
        selection_present: false,
    })
}

fn refresh_loop_should_continue(stop: &AtomicBool) -> bool {
    !stop.load(Ordering::Relaxed)
}

fn map_key(key: &Key) -> Option<TerminalKey> {
    match key {
        Key::Character(character) => character.chars().next().map(TerminalKey::Character),
        Key::Named(NamedKey::Enter) => Some(TerminalKey::Enter),
        Key::Named(NamedKey::Backspace) => Some(TerminalKey::Backspace),
        Key::Named(NamedKey::Escape) => Some(TerminalKey::Escape),
        Key::Named(NamedKey::Tab) => Some(TerminalKey::Tab),
        Key::Named(NamedKey::Space) => Some(TerminalKey::Character(' ')),
        _ => None,
    }
}

fn nonblank_terminal_text(text: String) -> String {
    if text.trim().is_empty() {
        "umux terminal MVP".to_string()
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_status_line_mentions_shell_and_size() {
        let line = terminal_status_line("pwsh", 80, 24);

        assert_eq!(line, "pwsh 80x24");
    }

    #[test]
    fn terminal_ui_state_has_nonblank_initial_text() {
        let state = TerminalUiState::initial("pwsh", "umux terminal MVP");

        assert_eq!(state.status, "pwsh 80x24");
        assert_eq!(state.text, "umux terminal MVP");
    }

    #[test]
    fn refresh_loop_stops_when_flag_is_set() {
        let stop = std::sync::atomic::AtomicBool::new(false);

        assert!(refresh_loop_should_continue(&stop));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);

        assert!(!refresh_loop_should_continue(&stop));
    }
}

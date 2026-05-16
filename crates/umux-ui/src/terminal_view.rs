// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TryRecvError, TrySendError};
use floem::Clipboard;
use floem::event::{Event, EventListener, EventPropagation};
use floem::ext_event::create_signal_from_channel;
use floem::keyboard::{Key, Modifiers, NamedKey};
use floem::prelude::*;
use umux_app::TerminalEntry;
use umux_core::{AppModel, PaneId, SurfaceId, WorkspaceId};
#[cfg(test)]
use umux_core::ModelError;
#[cfg(windows)]
use umux_terminal::{LiveTerminalSession, PtySpawnConfig, ShellResolver};
use umux_terminal::{
    StartupEnvironment, TerminalCell, TerminalColor, TerminalCursor, TerminalHealth,
    TerminalInputRoute, TerminalInputRouter, TerminalKey, TerminalKeyEvent, TerminalMetrics,
    TerminalNotification, TerminalRendererSnapshot, TerminalSelection, TerminalStatus,
};

#[cfg(windows)]
type UiTerminalSession = LiveTerminalSession;

#[cfg(not(windows))]
struct UiTerminalSession;

enum TerminalSessionController {
    Internal {
        session: Arc<UiTerminalSession>,
        refresh_stop: Arc<AtomicBool>,
    },
    External {
        entry: Arc<TerminalEntry>,
        refresh_stop: Arc<AtomicBool>,
    },
}

impl TerminalSessionController {
    fn send_input(&self, input: impl AsRef<[u8]>) {
        match self {
            Self::Internal { session, .. } => {
                let _ = session.send_input(input);
            }
            Self::External { entry, .. } => entry.send_input(input),
        }
    }

    fn resize(&self, cols: u16, rows: u16) {
        match self {
            Self::Internal { session, .. } => {
                let _ = session.resize(cols, rows);
            }
            Self::External { entry, .. } => entry.resize(cols, rows),
        }
    }

    fn drain_notifications(&self) -> Vec<TerminalNotification> {
        match self {
            Self::Internal { session, .. } => session.drain_notifications(),
            Self::External { entry, .. } => entry.drain_notifications(),
        }
    }

    fn snapshot(&self) -> Option<TerminalRendererSnapshot> {
        match self {
            #[cfg(windows)]
            Self::Internal { session, .. } => Some(session.snapshot()),
            #[cfg(not(windows))]
            Self::Internal { .. } => None,
            Self::External { entry, .. } => entry.snapshot(),
        }
    }

    fn health(&self) -> Option<TerminalHealth> {
        match self {
            #[cfg(windows)]
            Self::Internal { session, .. } => Some(session.health()),
            #[cfg(not(windows))]
            Self::Internal { .. } => None,
            Self::External { entry, .. } => entry.health(),
        }
    }

    fn is_alive(&self) -> bool {
        match self {
            #[cfg(windows)]
            Self::Internal { session, .. } => session.is_alive(),
            #[cfg(not(windows))]
            Self::Internal { .. } => true,
            Self::External { entry, .. } => match entry.as_ref() {
                TerminalEntry::Failed { .. } => false,
                _ => entry.health().is_none_or(|health| {
                    !matches!(
                        health.status,
                        TerminalStatus::Exited | TerminalStatus::Failed
                    )
                }),
            },
        }
    }
}

impl Drop for TerminalSessionController {
    fn drop(&mut self) {
        match self {
            Self::Internal { refresh_stop, .. } | Self::External { refresh_stop, .. } => {
                refresh_stop.store(true, Ordering::Relaxed);
            }
        }
    }
}

type TerminalSessionHandle = Arc<TerminalSessionController>;

const TERMINAL_BG: Color = Color::rgb8(0x11, 0x13, 0x16);
const TERMINAL_MUTED: Color = Color::rgb8(0x9b, 0xa3, 0xaf);

pub fn terminal_status_line(shell: &str, cols: u16, rows: u16) -> String {
    format!("{shell} {cols}x{rows}")
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalUiState {
    status: String,
    snapshot: TerminalRendererSnapshot,
}

#[derive(Clone)]
struct TerminalUiStateSink {
    tx: Sender<TerminalUiState>,
    coalesce_rx: Receiver<TerminalUiState>,
}

impl TerminalUiState {
    fn initial(shell: &str, text: &str) -> Self {
        Self {
            status: terminal_status_line(shell, 80, 24),
            snapshot: snapshot_from_text(text, 80, 24),
        }
    }

    fn from_health(health: TerminalHealth, snapshot: TerminalRendererSnapshot) -> Self {
        Self {
            status: terminal_status_line(&health.shell, health.cols, health.rows),
            snapshot,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TerminalLaunchContext {
    cwd: String,
    workspace_id: WorkspaceId,
    pane_id: PaneId,
    surface_id: SurfaceId,
}

impl TerminalLaunchContext {
    #[cfg(test)]
    pub(crate) fn from_model(model: &AppModel) -> Result<Self, ModelError> {
        let workspace = model.selected_workspace()?;
        let pane = workspace
            .selected_pane()
            .ok_or(ModelError::NoSelectedPane)?;

        Ok(Self {
            cwd: pane.cwd.clone(),
            workspace_id: workspace.id,
            pane_id: pane.id,
            surface_id: pane.selected_surface,
        })
    }

    pub(crate) fn fallback(cwd: String) -> Self {
        Self {
            cwd,
            workspace_id: WorkspaceId(1),
            pane_id: PaneId(1),
            surface_id: SurfaceId(1),
        }
    }

    fn from_entry(entry: &TerminalEntry) -> Self {
        let spec = entry.spec();
        Self {
            cwd: spec.cwd.clone(),
            workspace_id: spec.workspace_id,
            pane_id: spec.pane_id,
            surface_id: spec.surface_id,
        }
    }

    fn startup_environment(&self) -> std::collections::HashMap<String, String> {
        StartupEnvironment::new(
            self.workspace_id.0,
            self.pane_id.0,
            self.surface_id.0,
            self.cwd.clone(),
        )
        .into_pairs()
    }
}

pub type SharedAppModel = Arc<Mutex<AppModel>>;

enum TerminalSessionSource {
    Internal(TerminalLaunchContext),
    External {
        context: TerminalLaunchContext,
        entry: Arc<TerminalEntry>,
    },
}

pub fn terminal_view() -> impl IntoView {
    terminal_view_for_cwd(".".to_string())
}

pub fn terminal_view_for_cwd(cwd: String) -> impl IntoView {
    terminal_view_for_context(TerminalLaunchContext::fallback(cwd), None)
}

pub(crate) fn terminal_view_for_context(
    context: TerminalLaunchContext,
    model: Option<SharedAppModel>,
) -> impl IntoView {
    terminal_view_for_source(TerminalSessionSource::Internal(context), model)
}

pub fn terminal_view_for_entry(
    entry: Arc<TerminalEntry>,
    model: Option<SharedAppModel>,
) -> impl IntoView {
    let context = TerminalLaunchContext::from_entry(&entry);
    terminal_view_for_source(TerminalSessionSource::External { context, entry }, model)
}

fn terminal_view_for_source(
    source: TerminalSessionSource,
    model: Option<SharedAppModel>,
) -> impl IntoView {
    let initial = TerminalUiState::initial("pwsh", "umux terminal MVP");
    let initial_status = initial.status.clone();
    let initial_snapshot = initial.snapshot.clone();
    let (state_tx, state_rx) = terminal_ui_state_channel();
    let state = create_signal_from_channel(state_rx);
    let _ = send_terminal_ui_state(&state_tx, initial.clone());
    let selection = create_rw_signal(None::<TerminalSelection>);
    let dragging = create_rw_signal(false);
    let metrics = TerminalMetrics::new(8.0, 16.0);

    let session = start_terminal_session_for_source(source, state_tx, model);
    let session_for_key = session.clone();
    let session_for_grid_resize = session.clone();
    let grid_chrome = TerminalGridChrome::default();
    let grid = dyn_stack(
        move || {
            let snapshot = state
                .get()
                .map(|state| state.snapshot)
                .unwrap_or_else(|| initial_snapshot.clone());
            render_rows(snapshot_with_selection(snapshot, selection.get()))
        },
        |row| row.key.clone(),
        move |row| {
            h_stack_from_iter(row.cells.into_iter().map(render_cell_view))
                .style(move |s| s.height(metrics.cell_height_px() as f64))
        },
    )
    .on_event_stop(EventListener::PointerDown, move |event| {
        let Event::PointerDown(event) = event else {
            return;
        };
        if !event.button.is_primary() {
            return;
        }
        let Some((col, row)) = pointer_to_grid_cell(event.pos.x, event.pos.y, metrics, grid_chrome)
        else {
            return;
        };
        dragging.set(true);
        selection.set(Some(TerminalSelection {
            start_col: col,
            start_row: row,
            end_col: col,
            end_row: row,
        }));
    })
    .on_event_stop(EventListener::PointerMove, move |event| {
        if !dragging.get_untracked() {
            return;
        }
        let Event::PointerMove(event) = event else {
            return;
        };
        let Some((col, row)) = pointer_to_grid_cell(event.pos.x, event.pos.y, metrics, grid_chrome)
        else {
            return;
        };
        if let Some(mut current) = selection.get_untracked() {
            current.end_col = col;
            current.end_row = row;
            selection.set(Some(current));
        }
    })
    .on_resize(move |rect| {
        let Some(session) = &session_for_grid_resize else {
            return;
        };
        let size =
            terminal_grid_size_for_viewport(metrics, rect.width() as f32, rect.height() as f32);
        session.resize(size.cols, size.rows);
    })
    .on_event_stop(EventListener::PointerUp, move |event| {
        dragging.set(false);
        let Event::PointerUp(event) = event else {
            return;
        };
        let Some((col, row)) = pointer_to_grid_cell(event.pos.x, event.pos.y, metrics, grid_chrome)
        else {
            return;
        };
        if let Some(mut current) = selection.get_untracked() {
            current.end_col = col;
            current.end_row = row;
            selection.set(Some(current));
        }
    });

    v_stack((
        label(move || {
            state
                .get()
                .map(|state| state.status)
                .unwrap_or_else(|| initial_status.clone())
        })
        .style(|s| s.color(TERMINAL_MUTED).font_size(12.0)),
        grid,
    ))
    .keyboard_navigable()
    .on_event(EventListener::KeyDown, move |event| {
        let Event::KeyDown(event) = event else {
            return EventPropagation::Continue;
        };
        let Some(session) = &session_for_key else {
            return EventPropagation::Continue;
        };

        let snapshot = state
            .get()
            .map(|state| snapshot_with_selection(state.snapshot, selection.get()))
            .unwrap_or_else(|| snapshot_from_text("", 1, 1));
        let route = route_key_event(event, snapshot.selection.is_some());
        let propagation = terminal_route_propagation(&route);
        match route {
            TerminalInputRoute::WriteBytes(bytes) => session.send_input(bytes),
            TerminalInputRoute::CopySelection => {
                if let Some(text) = snapshot.selected_text() {
                    let _ = Clipboard::set_contents(text);
                }
            }
            TerminalInputRoute::PasteClipboard => {
                if let Ok(text) = Clipboard::get_contents() {
                    session.send_input(text.into_bytes());
                }
            }
            TerminalInputRoute::Ignore => {}
        }
        propagation
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

fn start_terminal_session_for_source(
    source: TerminalSessionSource,
    state_tx: TerminalUiStateSink,
    model: Option<SharedAppModel>,
) -> Option<TerminalSessionHandle> {
    match source {
        TerminalSessionSource::Internal(context) => {
            start_terminal_session(context, state_tx, model)
        }
        TerminalSessionSource::External { context, entry } => Some(
            start_external_terminal_session(context, entry, state_tx, model),
        ),
    }
}

fn start_external_terminal_session(
    context: TerminalLaunchContext,
    entry: Arc<TerminalEntry>,
    state_tx: TerminalUiStateSink,
    model: Option<SharedAppModel>,
) -> TerminalSessionHandle {
    let surface_id = context.surface_id;
    let _ = send_terminal_ui_state(&state_tx, initial_state_for_entry(&entry));
    let controller = Arc::new(TerminalSessionController::External {
        entry,
        refresh_stop: Arc::new(AtomicBool::new(false)),
    });
    if controller.snapshot().is_none() && controller.health().is_none() {
        return controller;
    }

    let refresh_stop = match controller.as_ref() {
        TerminalSessionController::External { refresh_stop, .. } => refresh_stop.clone(),
        TerminalSessionController::Internal { .. } => unreachable!(),
    };
    let refresh_session = Arc::downgrade(&controller);

    std::thread::spawn(move || {
        while refresh_loop_should_continue(&refresh_stop) {
            let Some(controller) = refresh_session.upgrade() else {
                return;
            };
            let keep_running = apply_terminal_refresh_result(
                &model,
                surface_id,
                controller.is_alive(),
                controller.drain_notifications(),
            );
            if let (Some(health), Some(snapshot)) = (controller.health(), controller.snapshot())
                && !send_terminal_ui_state(
                    &state_tx,
                    TerminalUiState::from_health(health, snapshot),
                )
            {
                return;
            }
            if !keep_running {
                break;
            }
            std::thread::sleep(Duration::from_millis(33));
        }

        if let Some(controller) = refresh_session.upgrade()
            && let (Some(health), Some(snapshot)) = (controller.health(), controller.snapshot())
        {
            let _ =
                send_terminal_ui_state(&state_tx, TerminalUiState::from_health(health, snapshot));
        }
    });

    controller
}

fn initial_state_for_entry(entry: &TerminalEntry) -> TerminalUiState {
    if let (Some(health), Some(snapshot)) = (entry.health(), entry.snapshot()) {
        return TerminalUiState::from_health(health, snapshot);
    }

    match entry {
        TerminalEntry::Failed { message, .. } => TerminalUiState {
            status: "terminal failed".to_string(),
            snapshot: snapshot_from_text(&format!("Unable to start terminal: {message}"), 80, 24),
        },
        _ => TerminalUiState::initial(
            "shell",
            "umux terminal MVP\nlive terminal is available on Windows",
        ),
    }
}

#[cfg(windows)]
fn start_terminal_session(
    context: TerminalLaunchContext,
    state_tx: TerminalUiStateSink,
    model: Option<SharedAppModel>,
) -> Option<TerminalSessionHandle> {
    let shell = ShellResolver::from_path().resolve();
    let env = context.startup_environment();
    let surface_id = context.surface_id;
    let config = PtySpawnConfig {
        shell,
        cwd: context.cwd,
        env,
        cols: 80,
        rows: 24,
    };

    let session = match LiveTerminalSession::spawn(config) {
        Ok(session) => session,
        Err(error) => {
            let _ = send_terminal_ui_state(
                &state_tx,
                TerminalUiState {
                    status: "terminal failed".to_string(),
                    snapshot: snapshot_from_text(
                        &format!("Unable to start terminal: {error}"),
                        80,
                        24,
                    ),
                },
            );
            return None;
        }
    };

    let controller = Arc::new(TerminalSessionController::Internal {
        session: Arc::new(session),
        refresh_stop: Arc::new(AtomicBool::new(false)),
    });
    let refresh_stop = match controller.as_ref() {
        TerminalSessionController::Internal { refresh_stop, .. } => refresh_stop.clone(),
        TerminalSessionController::External { .. } => unreachable!(),
    };
    let refresh_session = Arc::downgrade(&controller);
    std::thread::spawn(move || {
        while refresh_loop_should_continue(&refresh_stop) {
            let Some(controller) = refresh_session.upgrade() else {
                return;
            };
            let keep_running = apply_terminal_refresh_result(
                &model,
                surface_id,
                controller.is_alive(),
                controller.drain_notifications(),
            );
            if !keep_running {
                break;
            }

            let Some(snapshot) = controller.snapshot() else {
                return;
            };
            let Some(health) = controller.health() else {
                return;
            };
            if !send_terminal_ui_state(&state_tx, TerminalUiState::from_health(health, snapshot)) {
                return;
            }
            std::thread::sleep(Duration::from_millis(33));
        }

        if let Some(controller) = refresh_session.upgrade()
            && let (Some(health), Some(snapshot)) = (controller.health(), controller.snapshot())
        {
            let _ =
                send_terminal_ui_state(&state_tx, TerminalUiState::from_health(health, snapshot));
        }
    });

    Some(controller)
}

#[cfg(not(windows))]
fn start_terminal_session(
    _context: TerminalLaunchContext,
    state_tx: TerminalUiStateSink,
    _model: Option<SharedAppModel>,
) -> Option<TerminalSessionHandle> {
    let _ = send_terminal_ui_state(
        &state_tx,
        TerminalUiState::initial(
            "shell",
            "umux terminal MVP\nlive terminal is available on Windows",
        ),
    );
    Some(Arc::new(TerminalSessionController::Internal {
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

    fn drain_notifications(&self) -> Vec<TerminalNotification> {
        Vec::new()
    }
}

fn route_key_event(
    event: &floem::keyboard::KeyEvent,
    selection_present: bool,
) -> TerminalInputRoute {
    let Some(key) = map_key(&event.key.logical_key) else {
        return TerminalInputRoute::Ignore;
    };

    TerminalInputRouter::route_key(TerminalKeyEvent {
        key,
        ctrl: event.modifiers.contains(Modifiers::CONTROL),
        shift: event.modifiers.contains(Modifiers::SHIFT),
        alt: event.modifiers.contains(Modifiers::ALT),
        selection_present,
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

fn apply_terminal_notifications(
    model: &mut AppModel,
    surface_id: SurfaceId,
    notifications: Vec<TerminalNotification>,
) {
    for notification in notifications {
        let _ = model.mark_surface_unread(surface_id, notification.message);
    }
}

fn apply_terminal_refresh_result(
    model: &Option<SharedAppModel>,
    surface_id: SurfaceId,
    keep_running: bool,
    notifications: Vec<TerminalNotification>,
) -> bool {
    if let Some(model) = model {
        let mut model = model.lock().expect("app model lock poisoned");
        apply_terminal_notifications(&mut model, surface_id, notifications);
    }

    keep_running
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalUiRow {
    key: String,
    cells: Vec<TerminalUiCell>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TerminalUiCell {
    key: String,
    ch: char,
    fg: TerminalColor,
    bg: TerminalColor,
    cursor: bool,
    selected: bool,
}

fn render_rows(snapshot: TerminalRendererSnapshot) -> Vec<TerminalUiRow> {
    let cols = usize::from(snapshot.cols);
    if cols == 0 {
        return Vec::new();
    }

    snapshot
        .render_cells()
        .chunks(cols)
        .enumerate()
        .map(|(row, cells)| {
            let cells = cells
                .iter()
                .map(|cell| TerminalUiCell {
                    key: format!(
                        "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
                        snapshot.version,
                        cell.row,
                        cell.col,
                        cell.cell.ch,
                        cell.cell.fg.r,
                        cell.cell.fg.g,
                        cell.cell.fg.b,
                        cell.cell.bg.r,
                        cell.cell.bg.g,
                        cell.cell.bg.b,
                        (cell.selected as u8) + ((cell.cursor as u8) << 1),
                    ),
                    ch: cell.cell.ch,
                    fg: cell.cell.fg,
                    bg: cell.cell.bg,
                    cursor: cell.cursor,
                    selected: cell.selected,
                })
                .collect::<Vec<_>>();
            let key = cells
                .iter()
                .map(|cell| cell.key.as_str())
                .collect::<Vec<_>>()
                .join("|");
            TerminalUiRow {
                key: format!("{row}:{key}"),
                cells,
            }
        })
        .collect()
}

fn render_cell_view(cell: TerminalUiCell) -> impl IntoView {
    let ch = cell.ch.to_string();
    let fg = cell_fg(&cell);
    let bg = cell_bg(&cell);

    container(
        label(move || ch.clone())
            .style(move |s| s.color(color_to_floem(fg)).font_size(13.0).line_height(1.0)),
    )
    .style(move |s| {
        s.width(8.0)
            .height(16.0)
            .items_center()
            .justify_center()
            .background(color_to_floem(bg))
    })
}

fn cell_fg(cell: &TerminalUiCell) -> TerminalColor {
    if cell.cursor {
        TerminalColor::rgb(0x11, 0x13, 0x16)
    } else {
        cell.fg
    }
}

fn cell_bg(cell: &TerminalUiCell) -> TerminalColor {
    if cell.cursor {
        TerminalColor::rgb(0xe7, 0xea, 0xf0)
    } else if cell.selected {
        TerminalColor::rgb(0x2f, 0x80, 0xff)
    } else {
        cell.bg
    }
}

fn color_to_floem(color: TerminalColor) -> Color {
    Color::rgb8(color.r, color.g, color.b)
}

fn terminal_ui_state_channel() -> (TerminalUiStateSink, Receiver<TerminalUiState>) {
    let (tx, rx) = crossbeam_channel::bounded(1);
    (
        TerminalUiStateSink {
            tx,
            coalesce_rx: rx.clone(),
        },
        rx,
    )
}

fn send_terminal_ui_state(sink: &TerminalUiStateSink, state: TerminalUiState) -> bool {
    match sink.tx.try_send(state) {
        Ok(()) => true,
        Err(TrySendError::Full(state)) => {
            drain_pending_terminal_state(&sink.coalesce_rx);
            match sink.tx.try_send(state) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            }
        }
        Err(TrySendError::Disconnected(_)) => false,
    }
}

fn drain_pending_terminal_state(rx: &Receiver<TerminalUiState>) {
    loop {
        match rx.try_recv() {
            Ok(_) => {}
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => return,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct TerminalGridChrome {
    padding_left: f64,
    padding_top: f64,
}

fn pointer_to_grid_cell(
    x: f64,
    y: f64,
    metrics: TerminalMetrics,
    chrome: TerminalGridChrome,
) -> Option<(u16, u16)> {
    let grid_x = x - chrome.padding_left;
    let grid_y = y - chrome.padding_top;
    if grid_x.is_sign_negative() || grid_y.is_sign_negative() {
        return None;
    }

    Some((
        (grid_x as f32 / metrics.cell_width_px()).floor() as u16,
        (grid_y as f32 / metrics.cell_height_px()).floor() as u16,
    ))
}

fn terminal_grid_size_for_viewport(
    metrics: TerminalMetrics,
    width_px: f32,
    height_px: f32,
) -> umux_terminal::TerminalGridSize {
    metrics.cols_rows(width_px, height_px)
}

fn snapshot_with_selection(
    mut snapshot: TerminalRendererSnapshot,
    selection: Option<TerminalSelection>,
) -> TerminalRendererSnapshot {
    snapshot.selection = selection;
    snapshot
}

fn snapshot_from_text(text: &str, cols: u16, rows: u16) -> TerminalRendererSnapshot {
    let cols = cols.max(1);
    let rows = rows.max(1);
    let mut cells = vec![terminal_cell(' '); usize::from(cols) * usize::from(rows)];
    for (row, line) in text.lines().take(usize::from(rows)).enumerate() {
        for (col, ch) in line.chars().take(usize::from(cols)).enumerate() {
            cells[row * usize::from(cols) + col] = terminal_cell(ch);
        }
    }

    TerminalRendererSnapshot {
        cols,
        rows,
        cells,
        cursor: TerminalCursor {
            col: 0,
            row: 0,
            visible: false,
        },
        selection: None,
        scrollback_lines: 0,
        version: 0,
    }
}

fn terminal_route_propagation(route: &TerminalInputRoute) -> EventPropagation {
    match route {
        TerminalInputRoute::Ignore => EventPropagation::Continue,
        TerminalInputRoute::WriteBytes(_)
        | TerminalInputRoute::CopySelection
        | TerminalInputRoute::PasteClipboard => EventPropagation::Stop,
    }
}

fn terminal_cell(ch: char) -> TerminalCell {
    TerminalCell {
        ch,
        fg: TerminalColor::rgb(0xe7, 0xea, 0xf0),
        bg: TerminalColor::rgb(0x11, 0x13, 0x16),
        bold: false,
        italic: false,
        underline: false,
        inverse: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use umux_core::AppModel;

    #[test]
    fn terminal_status_line_mentions_shell_and_size() {
        let line = terminal_status_line("pwsh", 80, 24);

        assert_eq!(line, "pwsh 80x24");
    }

    #[test]
    fn terminal_ui_state_has_nonblank_initial_text() {
        let state = TerminalUiState::initial("pwsh", "umux terminal MVP");

        assert_eq!(state.status, "pwsh 80x24");
        assert_eq!(
            state.snapshot.visible_text().lines().next(),
            Some("umux terminal MVP")
        );
    }

    #[cfg(windows)]
    #[test]
    fn terminal_ui_state_from_health_preserves_blank_real_snapshot() {
        let snapshot = snapshot_from_text("", 80, 24);
        let state = TerminalUiState::from_health(
            umux_terminal::TerminalHealth::running("pwsh", "C:/work/alpha", 80, 24),
            snapshot,
        );

        assert!(state.snapshot.visible_text().trim().is_empty());
    }

    #[test]
    fn ignored_terminal_key_routes_continue_propagating() {
        assert!(terminal_route_propagation(&TerminalInputRoute::Ignore).is_continue());
        assert!(
            terminal_route_propagation(&TerminalInputRoute::WriteBytes(b"x".to_vec())).is_stop()
        );
    }

    #[test]
    fn refresh_loop_stops_when_flag_is_set() {
        let stop = std::sync::atomic::AtomicBool::new(false);

        assert!(refresh_loop_should_continue(&stop));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);

        assert!(!refresh_loop_should_continue(&stop));
    }

    #[test]
    fn launch_context_uses_selected_model_ids_for_startup_env() {
        let mut model = AppModel::new("C:/work/alpha");
        model
            .split_selected_pane(umux_core::SplitAxis::Vertical)
            .unwrap();
        let context = TerminalLaunchContext::from_model(&model).unwrap();
        let workspace_id = model.selected_workspace().unwrap().id;
        let pane = model.selected_pane().unwrap();

        assert_eq!(context.workspace_id, workspace_id);
        assert_eq!(context.pane_id, pane.id);
        assert_eq!(context.surface_id, pane.selected_surface);
        assert_eq!(context.cwd, pane.cwd);
        let env = context.startup_environment();
        let expected_surface_id = pane.selected_surface.0.to_string();
        assert_eq!(
            env.get("UMUX_SURFACE_ID").map(String::as_str),
            Some(expected_surface_id.as_str())
        );
    }

    #[test]
    fn terminal_ui_applies_notifications_to_real_surface_id() {
        let mut model = AppModel::new("C:/work/alpha");
        let surface_id = model.selected_pane().unwrap().selected_surface;
        let mut emulator = umux_terminal::TerminalEmulator::new(20, 3, 100);
        let notifications = emulator.feed_bytes(b"\x1b]9;Build done\x07");

        apply_terminal_notifications(&mut model, surface_id, notifications);

        let workspace = model.selected_workspace().unwrap();
        assert!(workspace.unread);
        assert_eq!(workspace.latest_notification.as_deref(), Some("Build done"));
        assert!(
            workspace
                .selected_pane()
                .unwrap()
                .surface(surface_id)
                .unwrap()
                .unread
        );
    }

    #[test]
    fn terminal_refresh_applies_pending_notifications_even_when_session_exited() {
        let model = Arc::new(Mutex::new(AppModel::new("C:/work/alpha")));
        let surface_id = model
            .lock()
            .unwrap()
            .selected_pane()
            .unwrap()
            .selected_surface;
        let mut emulator = umux_terminal::TerminalEmulator::new(20, 3, 100);
        let notifications = emulator.feed_bytes(b"\x1b]9;done before exit\x07");

        let keep_running =
            apply_terminal_refresh_result(&Some(model.clone()), surface_id, false, notifications);

        assert!(!keep_running);
        let model = model.lock().unwrap();
        let workspace = model.selected_workspace().unwrap();
        assert_eq!(
            workspace.latest_notification.as_deref(),
            Some("done before exit")
        );
        assert!(
            workspace
                .selected_pane()
                .unwrap()
                .surface(surface_id)
                .unwrap()
                .unread
        );
    }

    #[test]
    fn pointer_to_grid_cell_subtracts_terminal_chrome_before_mapping() {
        let metrics = TerminalMetrics::new(8.0, 16.0);
        let chrome = TerminalGridChrome {
            padding_left: 12.0,
            padding_top: 36.0,
        };

        assert_eq!(
            pointer_to_grid_cell(28.0, 68.0, metrics, chrome),
            Some((2, 2))
        );
        assert_eq!(pointer_to_grid_cell(11.0, 68.0, metrics, chrome), None);
        assert_eq!(pointer_to_grid_cell(28.0, 35.0, metrics, chrome), None);
    }

    #[test]
    fn resize_uses_grid_viewport_size_without_outer_chrome() {
        let metrics = TerminalMetrics::new(8.0, 16.0);

        let size = terminal_grid_size_for_viewport(metrics, 80.0, 32.0);

        assert_eq!(size.cols, 10);
        assert_eq!(size.rows, 2);
    }

    #[test]
    fn terminal_state_sender_coalesces_to_latest_pending_state() {
        let (sink, rx) = terminal_ui_state_channel();

        assert!(send_terminal_ui_state(
            &sink,
            TerminalUiState::initial("pwsh", "first")
        ));
        assert!(send_terminal_ui_state(
            &sink,
            TerminalUiState::initial("pwsh", "second")
        ));

        assert_eq!(
            rx.try_recv()
                .unwrap()
                .snapshot
                .visible_text()
                .lines()
                .next(),
            Some("second")
        );
        assert!(rx.try_recv().is_err());
    }
}

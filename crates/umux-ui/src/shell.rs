// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::sync::{Arc, Mutex};

use floem::event::{Event, EventListener, EventPropagation};
use floem::ext_event::create_signal_from_channel;
use floem::keyboard::{Key, NamedKey};
use floem::prelude::*;
use floem::reactive::create_effect;
use floem::style::Style;
use tracing::{info, warn};
use umux_app::session_store::SessionLoadOutcome;
use umux_app::{
    AppAction, AppController, AppControllerError, SessionStore, SessionStoreError, TerminalEntry,
};
use umux_config::default_shortcuts;
use umux_core::AppModel;
use umux_core::model::{SplitTree, Workspace};
use umux_core::{PaneId, SplitAxis, SurfaceId, SurfaceKind, WorkspaceId};

use crate::terminal_view::{
    SharedAppModel, TerminalNotificationEvent, TerminalNotificationSink,
    terminal_view_for_entry_with_notifications,
};
use crate::theme::{SIDEBAR_WIDTH, SURFACE_TAB_HEIGHT, TOP_BAR_HEIGHT};

const BACKGROUND: Color = Color::rgb8(0x11, 0x13, 0x16);
const PANEL: Color = Color::rgb8(0x18, 0x1b, 0x20);
const TEXT: Color = Color::rgb8(0xe7, 0xea, 0xf0);
const MUTED_TEXT: Color = Color::rgb8(0x9b, 0xa3, 0xaf);
const UNREAD_BLUE: Color = Color::rgb8(0x2f, 0x80, 0xff);
const RECOVERED_SESSION_WARNING: &str =
    "Previous session could not be restored. A recovered copy was moved aside.";
const SESSION_READ_WARNING: &str = "Session file could not be read. Opened a fresh workspace.";
const RESTORE_CONTROLLER_WARNING: &str =
    "Previous session could not be restored. Opened a fresh workspace.";

fn workspace_row_label(workspace: &umux_core::model::Workspace) -> String {
    if workspace.unread {
        format!("{} *", workspace.title)
    } else {
        workspace.title.clone()
    }
}

fn surface_tab_label(surface: &umux_core::model::Surface) -> String {
    if surface.unread {
        format!("{} *", surface.title)
    } else {
        surface.title.clone()
    }
}

pub fn run() {
    crate::diagnostics::init_diagnostics();
    floem::launch(app_view);
}

pub fn seed_model() -> AppModel {
    AppModel::new(current_dir_cwd())
}

fn current_dir_cwd() -> String {
    env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

#[derive(Clone)]
struct StartupState {
    controller: AppController,
    warning: Option<StartupWarning>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StartupWarning {
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StartupLoadDecision {
    model: AppModel,
    restored: bool,
    warning: Option<StartupWarning>,
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
                warning: Some(StartupWarning {
                    message: RECOVERED_SESSION_WARNING.to_string(),
                }),
            }
        }
        Err(error) => {
            warn!(%error, "saved session could not be read");
            StartupLoadDecision {
                model: AppModel::new(fallback_cwd),
                restored: false,
                warning: Some(StartupWarning {
                    message: SESSION_READ_WARNING.to_string(),
                }),
            }
        }
    }
}

fn startup_state_from_store(store: &SessionStore) -> StartupState {
    startup_state_from_decision(startup_load_decision(
        store.load_model_with_status(),
        current_dir_cwd(),
    ))
}

fn startup_state_from_decision(decision: StartupLoadDecision) -> StartupState {
    match AppController::from_restored_model(decision.model) {
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
        controller: AppController::new(seed_model())
            .expect("seed model should create an app controller"),
        warning: Some(StartupWarning {
            message: RESTORE_CONTROLLER_WARNING.to_string(),
        }),
    }
}

fn app_view() -> impl IntoView {
    let store = SessionStore::new(SessionStore::default_path());
    let startup = startup_state_from_store(&store);

    shell_view(startup, store)
}

fn shell_view(startup: StartupState, store: SessionStore) -> impl IntoView {
    let shared_model = Arc::new(Mutex::new(startup.controller.model.clone()));
    let controller = create_rw_signal(startup.controller);
    let store = Arc::new(store);
    let startup_warning = startup.warning;
    let (notification_tx, notification_rx) = crossbeam_channel::unbounded();
    let terminal_events = create_signal_from_channel(notification_rx);
    {
        let store = store.clone();
        let shared_model = shared_model.clone();
        create_effect(move |_| {
            if let Some(event) = terminal_events.get() {
                controller.update(|controller| {
                    apply_terminal_notification_event(controller, store.as_ref(), event);
                    sync_model_mirror(controller, &shared_model);
                });
            }
        });
    }

    app_shell(
        controller,
        store,
        shared_model,
        notification_tx,
        startup_warning,
    )
}

fn dispatch_action(controller: &mut AppController, store: &SessionStore, action: AppAction) {
    if controller.apply(action).is_ok() {
        save_session(store, &controller.model, "dispatch_action");
    }
}

fn dispatch_actions(
    controller: &mut AppController,
    store: &SessionStore,
    actions: impl IntoIterator<Item = AppAction>,
) {
    let mut changed = false;
    for action in actions {
        changed |= controller.apply(action).is_ok();
    }

    if changed {
        save_session(store, &controller.model, "dispatch_actions");
    }
}

fn dispatch_shell_action(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
    action: AppAction,
) {
    controller.update(move |controller| {
        dispatch_action(controller, store.as_ref(), action);
        sync_model_mirror(controller, &shared_model);
    });
}

fn dispatch_shell_actions(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
    actions: impl IntoIterator<Item = AppAction> + 'static,
) {
    controller.update(move |controller| {
        dispatch_actions(controller, store.as_ref(), actions);
        sync_model_mirror(controller, &shared_model);
    });
}

fn apply_terminal_notification_event(
    controller: &mut AppController,
    store: &SessionStore,
    event: TerminalNotificationEvent,
) -> bool {
    let mut changed = false;
    for notification in event.notifications {
        changed |= controller
            .model
            .mark_surface_unread(event.surface_id, notification.message)
            .is_ok();
    }

    if changed {
        save_session(store, &controller.model, "terminal_notification");
    }

    changed
}

fn save_session(store: &SessionStore, model: &AppModel, reason: &'static str) {
    if let Err(error) = store.save_model(model) {
        warn!(%error, %reason, "failed to save session");
    }
}

fn sync_model_mirror(controller: &AppController, shared_model: &SharedAppModel) {
    if let Ok(mut model) = shared_model.lock() {
        *model = controller.model.clone();
    }
}

fn app_shell(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
    notification_sink: TerminalNotificationSink,
    startup_warning: Option<StartupWarning>,
) -> impl IntoView {
    let shortcut_store = store.clone();
    let shortcut_shared_model = shared_model.clone();

    v_stack((
        top_bar(controller, store.clone(), shared_model.clone()),
        startup_warning_banner(startup_warning),
        h_stack((
            sidebar(controller, store.clone(), shared_model.clone()),
            work_area(controller, store, shared_model, notification_sink),
        ))
        .style(|s| s.flex().width_full().height_full().min_width(0.0)),
    ))
    .on_event(EventListener::KeyDown, move |event| {
        let Event::KeyDown(event) = event else {
            return EventPropagation::Continue;
        };
        let Some(chord) = chord_from_key_event(event) else {
            return EventPropagation::Continue;
        };
        let controller_snapshot = controller.get();
        let action = shell_action_for_shortcut(&controller_snapshot.model, &chord)
            .map(runtime_shortcut_action);
        drop(controller_snapshot);

        let Some(action) = action else {
            if let Some(action) = deferred_shortcut_action_for_chord(&chord) {
                warn!(%chord, %action, "shortcut action is not part of the current MVP surface");
            }
            return EventPropagation::Continue;
        };

        dispatch_shell_action(
            controller,
            shortcut_store.clone(),
            shortcut_shared_model.clone(),
            action,
        );
        EventPropagation::Stop
    })
    .style(|s| s.size_full().background(BACKGROUND).color(TEXT))
}

fn startup_warning_banner(warning: Option<StartupWarning>) -> impl IntoView {
    match warning {
        Some(warning) => {
            let message = startup_warning_banner_message(&warning);
            container(label(move || message.clone()))
                .style(|s| {
                    s.height(30.0)
                        .width_full()
                        .items_center()
                        .padding_horiz(14.0)
                        .background(Color::rgb8(0x33, 0x2a, 0x18))
                        .color(Color::rgb8(0xff, 0xdf, 0x9b))
                        .font_size(12.0)
                        .border_bottom(1.0)
                        .border_color(Color::rgb8(0x5c, 0x45, 0x20))
                })
                .into_any()
        }
        None => container(empty())
            .style(|s| s.height(0.0).width_full())
            .into_any(),
    }
}

fn startup_warning_banner_message(warning: &StartupWarning) -> String {
    warning.message.clone()
}

fn runtime_shortcut_action(action: AppAction) -> AppAction {
    match action {
        AppAction::NewWorkspace { cwd, title } if cwd == "." && title.is_none() => {
            AppAction::NewWorkspace {
                cwd: current_dir_cwd(),
                title,
            }
        }
        action => action,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShortcutDisposition {
    Shell,
    Terminal,
    Deferred,
    Unbound,
}

fn shortcut_disposition_for_action(action: &str) -> ShortcutDisposition {
    match action {
        "new_workspace"
        | "jump_workspace_1_8"
        | "jump_last_workspace"
        | "close_workspace"
        | "new_surface"
        | "close_surface"
        | "split_right"
        | "split_down"
        | "jump_latest_unread" => ShortcutDisposition::Shell,
        "copy" | "paste" => ShortcutDisposition::Terminal,
        "toggle_sidebar" | "open_browser_split" | "focus_address_bar" | "show_notifications"
        | "clear_scrollback" | "settings" => ShortcutDisposition::Deferred,
        _ => ShortcutDisposition::Unbound,
    }
}

fn deferred_shortcut_action_for_chord(chord: &str) -> Option<String> {
    default_shortcuts()
        .into_iter()
        .find(|binding| {
            shortcut_disposition_for_action(&binding.action) == ShortcutDisposition::Deferred
                && binding
                    .windows_bindings
                    .iter()
                    .any(|windows_binding| windows_binding.chord == chord)
        })
        .map(|binding| binding.action)
}

fn shell_action_for_shortcut(model: &AppModel, chord: &str) -> Option<AppAction> {
    match chord {
        "Ctrl+N" => Some(AppAction::NewWorkspace {
            cwd: ".".to_string(),
            title: None,
        }),
        "Ctrl+1" => workspace_at_index(model, 0).map(AppAction::SelectWorkspace),
        "Ctrl+2" => workspace_at_index(model, 1).map(AppAction::SelectWorkspace),
        "Ctrl+3" => workspace_at_index(model, 2).map(AppAction::SelectWorkspace),
        "Ctrl+4" => workspace_at_index(model, 3).map(AppAction::SelectWorkspace),
        "Ctrl+5" => workspace_at_index(model, 4).map(AppAction::SelectWorkspace),
        "Ctrl+6" => workspace_at_index(model, 5).map(AppAction::SelectWorkspace),
        "Ctrl+7" => workspace_at_index(model, 6).map(AppAction::SelectWorkspace),
        "Ctrl+8" => workspace_at_index(model, 7).map(AppAction::SelectWorkspace),
        "Ctrl+9" => last_workspace(model).map(AppAction::SelectWorkspace),
        "Ctrl+Shift+W" => model
            .selected_workspace()
            .ok()
            .map(|workspace| AppAction::CloseWorkspace(workspace.id)),
        "Ctrl+T" => Some(AppAction::NewTerminalTab),
        "Ctrl+W" => model
            .selected_pane()
            .ok()
            .map(|pane| AppAction::CloseSurface(pane.selected_surface)),
        "Ctrl+Alt+D" => Some(AppAction::SplitPane(SplitAxis::Vertical)),
        "Ctrl+Shift+Alt+D" => Some(AppAction::SplitPane(SplitAxis::Horizontal)),
        "Ctrl+Shift+U" => Some(AppAction::JumpLatestUnread),
        _ => None,
    }
}

fn workspace_at_index(model: &AppModel, index: usize) -> Option<WorkspaceId> {
    model
        .selected_window()
        .ok()
        .and_then(|window| window.workspaces.get(index))
        .map(|workspace| workspace.id)
}

fn last_workspace(model: &AppModel) -> Option<WorkspaceId> {
    model
        .selected_window()
        .ok()
        .and_then(|window| window.workspaces.last())
        .map(|workspace| workspace.id)
}

fn chord_from_key_event(event: &floem::keyboard::KeyEvent) -> Option<String> {
    if event.modifiers.meta() || event.modifiers.altgr() {
        return None;
    }

    let key = match &event.key.logical_key {
        Key::Character(character) => character.to_uppercase(),
        Key::Named(NamedKey::Enter) => "Enter".to_string(),
        _ => return None,
    };

    let mut parts = Vec::new();
    if event.modifiers.control() {
        parts.push("Ctrl");
    }
    if event.modifiers.shift() {
        parts.push("Shift");
    }
    if event.modifiers.alt() {
        parts.push("Alt");
    }
    parts.push(&key);

    Some(parts.join("+"))
}

fn top_bar(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    h_stack((
        label(|| "umux"),
        button(label(|| "jump"))
            .action(move || {
                dispatch_shell_action(
                    controller,
                    store.clone(),
                    shared_model.clone(),
                    AppAction::JumpLatestUnread,
                );
            })
            .style(compact_button_style),
    ))
    .style(|s| {
        s.height(TOP_BAR_HEIGHT)
            .width_full()
            .items_center()
            .justify_between()
            .padding_horiz(14.0)
            .background(BACKGROUND)
            .border_bottom(1.0)
            .border_color(Color::rgb8(0x25, 0x2a, 0x32))
            .font_size(12.0)
    })
}

fn sidebar(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    let row_store = store.clone();
    let row_shared_model = shared_model.clone();
    let new_workspace_store = store.clone();
    let new_workspace_shared_model = shared_model.clone();
    let close_workspace_store = store.clone();
    let close_workspace_shared_model = shared_model.clone();

    v_stack((
        label(|| "workspaces").style(|s| s.color(MUTED_TEXT).font_size(11.0)),
        dyn_stack(
            move || workspace_rows(controller),
            workspace_row_key,
            move |row| {
                workspace_row_button(row, controller, row_store.clone(), row_shared_model.clone())
            },
        )
        .style(|s| s.width_full().flex_col().gap(4.0)),
        button(label(|| "+ ws"))
            .action(move || {
                let cwd = current_dir_cwd();
                dispatch_shell_action(
                    controller,
                    new_workspace_store.clone(),
                    new_workspace_shared_model.clone(),
                    AppAction::NewWorkspace { cwd, title: None },
                );
            })
            .style(compact_button_style),
        button(label(|| "x ws"))
            .action(move || {
                if let Ok(workspace) = controller.get().model.selected_workspace() {
                    dispatch_shell_action(
                        controller,
                        close_workspace_store.clone(),
                        close_workspace_shared_model.clone(),
                        AppAction::CloseWorkspace(workspace.id),
                    );
                }
            })
            .style(compact_button_style),
    ))
    .style(|s| {
        s.width(SIDEBAR_WIDTH)
            .height_full()
            .padding(14.0)
            .gap(10.0)
            .background(PANEL)
            .border_right(1.0)
            .border_color(Color::rgb8(0x25, 0x2a, 0x32))
    })
}

fn work_area(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
    notification_sink: TerminalNotificationSink,
) -> impl IntoView {
    v_stack((
        workspace_controls(controller, store.clone(), shared_model.clone()),
        dyn_stack(
            move || pane_identity_rows_for_controller(controller),
            pane_row_key,
            move |pane| {
                pane_view(
                    pane,
                    controller,
                    store.clone(),
                    shared_model.clone(),
                    notification_sink.clone(),
                )
            },
        )
        .style(move |s| pane_stack_style(s, split_direction_for_controller(controller))),
    ))
    .style(|s| {
        s.width_full()
            .height_full()
            .min_width(0.0)
            .background(BACKGROUND)
    })
}

fn workspace_controls(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    let new_tab_store = store.clone();
    let new_tab_shared_model = shared_model.clone();
    let split_store = store.clone();
    let split_shared_model = shared_model.clone();
    let close_tab_store = store.clone();
    let close_tab_shared_model = shared_model.clone();
    let close_pane_store = store.clone();
    let close_pane_shared_model = shared_model.clone();

    h_stack((
        label(move || selected_workspace_title(controller))
            .style(|s| s.color(TEXT).font_size(13.0).font_bold().text_ellipsis()),
        h_stack((
            button(label(|| "+ tab"))
                .action(move || {
                    dispatch_shell_action(
                        controller,
                        new_tab_store.clone(),
                        new_tab_shared_model.clone(),
                        AppAction::NewTerminalTab,
                    );
                })
                .style(compact_button_style),
            button(label(|| "split"))
                .action(move || {
                    dispatch_shell_action(
                        controller,
                        split_store.clone(),
                        split_shared_model.clone(),
                        AppAction::SplitPane(SplitAxis::Vertical),
                    );
                })
                .style(compact_button_style),
            button(label(|| "x tab"))
                .action(move || {
                    if let Ok(pane) = controller.get().model.selected_pane() {
                        dispatch_shell_action(
                            controller,
                            close_tab_store.clone(),
                            close_tab_shared_model.clone(),
                            AppAction::CloseSurface(pane.selected_surface),
                        );
                    }
                })
                .style(compact_button_style),
            button(label(|| "x pane"))
                .action(move || {
                    if let Ok(pane) = controller.get().model.selected_pane() {
                        dispatch_shell_action(
                            controller,
                            close_pane_store.clone(),
                            close_pane_shared_model.clone(),
                            AppAction::ClosePane(pane.id),
                        );
                    }
                })
                .style(compact_button_style),
        ))
        .style(|s| s.items_center().gap(6.0)),
    ))
    .style(|s| {
        s.height(SURFACE_TAB_HEIGHT)
            .width_full()
            .items_center()
            .justify_between()
            .padding_horiz(12.0)
            .gap(12.0)
            .background(Color::rgb8(0x14, 0x17, 0x1b))
            .border_bottom(1.0)
            .border_color(Color::rgb8(0x25, 0x2a, 0x32))
    })
}

fn pane_view(
    pane: PaneRow,
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
    notification_sink: TerminalNotificationSink,
) -> impl IntoView {
    let pane_id = pane.id;
    let tab_shared_model = shared_model.clone();

    v_stack((
        dyn_stack(
            move || surface_tab_identity_rows(controller, pane_id),
            surface_tab_key,
            move |tab| {
                surface_tab_button(
                    tab,
                    pane_id,
                    controller,
                    store.clone(),
                    tab_shared_model.clone(),
                )
            },
        )
        .style(|s| {
            s.height(SURFACE_TAB_HEIGHT)
                .width_full()
                .items_center()
                .gap(4.0)
                .padding_horiz(8.0)
                .background(PANEL)
                .border_bottom(1.0)
                .border_color(Color::rgb8(0x25, 0x2a, 0x32))
        }),
        dyn_stack(
            move || terminal_content_rows(controller, pane_id),
            terminal_content_key,
            move |content| {
                terminal_content_view(content, shared_model.clone(), notification_sink.clone())
            },
        )
        .style(|s| s.width_full().height_full().min_height(0.0)),
    ))
    .style(move |s| {
        let border = if pane_selected(controller, pane.id) {
            UNREAD_BLUE
        } else {
            Color::rgb8(0x25, 0x2a, 0x32)
        };
        s.width_full()
            .height_full()
            .min_height(0.0)
            .background(BACKGROUND)
            .border_left(3.0)
            .border_color(border)
            .flex_basis(0.0)
            .flex_grow(1.0)
    })
}

fn terminal_content_view(
    content: TerminalContentRow,
    shared_model: SharedAppModel,
    notification_sink: TerminalNotificationSink,
) -> impl IntoView {
    match content.entry {
        Some(entry) => terminal_view_for_entry_with_notifications(
            Arc::new(entry),
            Some(shared_model),
            Some(notification_sink),
        )
        .style(|s| s.width_full().height_full().min_height(0.0))
        .into_any(),
        None => unavailable_terminal_view().into_any(),
    }
}

fn unavailable_terminal_view() -> impl IntoView {
    container(label(|| "Terminal unavailable").style(|s| s.color(MUTED_TEXT).font_size(12.0)))
        .style(|s| {
            s.width_full()
                .height_full()
                .items_center()
                .justify_center()
                .background(BACKGROUND)
        })
}

fn workspace_row_button(
    row: WorkspaceRow,
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    let workspace_id = row.id;
    let action = AppAction::SelectWorkspace(workspace_id);
    button(
        label(move || {
            workspace_row_state_for_controller(controller, workspace_id)
                .map(|state| state.label)
                .unwrap_or_else(|| "Workspace".to_string())
        })
        .style(|s| s.text_ellipsis()),
    )
    .action(move || {
        dispatch_shell_action(
            controller,
            store.clone(),
            shared_model.clone(),
            action.clone(),
        );
    })
    .style(move |s| {
        let background = if workspace_row_state_for_controller(controller, workspace_id)
            .is_some_and(|state| state.selected)
        {
            Color::rgb8(0x22, 0x28, 0x31)
        } else {
            PANEL
        };
        s.width_full()
            .height(28.0)
            .items_center()
            .justify_start()
            .padding_horiz(8.0)
            .background(background)
            .color(TEXT)
            .font_size(12.0)
            .border_radius(4.0)
    })
}

fn surface_tab_button(
    tab: SurfaceTabRow,
    pane_id: PaneId,
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    let surface_id = tab.id;
    button(
        label(move || {
            surface_tab_state_for_controller(controller, pane_id, surface_id)
                .map(|state| state.label)
                .unwrap_or_else(|| "Terminal".to_string())
        })
        .style(|s| s.text_ellipsis()),
    )
    .action(move || {
        dispatch_shell_actions(
            controller,
            store.clone(),
            shared_model.clone(),
            [
                AppAction::SelectPane(pane_id),
                AppAction::SelectSurface(surface_id),
            ],
        );
    })
    .style(move |s| {
        let background = if surface_tab_state_for_controller(controller, pane_id, surface_id)
            .is_some_and(|state| state.selected)
        {
            Color::rgb8(0x22, 0x28, 0x31)
        } else {
            Color::rgb8(0x14, 0x17, 0x1b)
        };
        s.height(24.0)
            .min_width(72.0)
            .max_width(160.0)
            .items_center()
            .padding_horiz(10.0)
            .background(background)
            .color(TEXT)
            .font_size(12.0)
            .border_radius(4.0)
    })
}

fn compact_button_style(s: Style) -> Style {
    s.height(24.0)
        .min_width(44.0)
        .items_center()
        .justify_center()
        .padding_horiz(8.0)
        .background(Color::rgb8(0x22, 0x28, 0x31))
        .color(TEXT)
        .font_size(12.0)
        .border_radius(4.0)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceRow {
    id: WorkspaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceRowState {
    label: String,
    selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneRow {
    id: PaneId,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct PaneLayout {
    direction: PaneStackDirection,
    panes: Vec<PaneRow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PaneStackDirection {
    Row,
    Column,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SurfaceTabRow {
    id: SurfaceId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SurfaceTabState {
    label: String,
    selected: bool,
}

#[derive(Clone)]
struct TerminalContentRow {
    surface_id: SurfaceId,
    entry: Option<TerminalEntry>,
}

fn workspace_row_key(row: &WorkspaceRow) -> WorkspaceId {
    row.id
}

fn pane_row_key(row: &PaneRow) -> PaneId {
    row.id
}

fn surface_tab_key(row: &SurfaceTabRow) -> SurfaceId {
    row.id
}

fn terminal_content_key(row: &TerminalContentRow) -> SurfaceId {
    row.surface_id
}

fn workspace_rows(controller: RwSignal<AppController>) -> Vec<WorkspaceRow> {
    controller
        .get()
        .model
        .selected_window()
        .ok()
        .map(|window| {
            window
                .workspaces
                .iter()
                .map(|workspace| WorkspaceRow { id: workspace.id })
                .collect()
        })
        .unwrap_or_default()
}

fn workspace_row_state_for_controller(
    controller: RwSignal<AppController>,
    workspace_id: WorkspaceId,
) -> Option<WorkspaceRowState> {
    workspace_row_state(&controller.get().model, workspace_id)
}

fn workspace_row_state(model: &AppModel, workspace_id: WorkspaceId) -> Option<WorkspaceRowState> {
    let window = model.selected_window().ok()?;
    let workspace = window.workspace(workspace_id)?;
    Some(WorkspaceRowState {
        label: workspace_row_label(workspace),
        selected: window.selected_workspace == workspace_id,
    })
}

#[cfg(test)]
fn pane_layout_for_workspace(workspace: &Workspace) -> PaneLayout {
    PaneLayout {
        direction: split_direction_for_workspace(workspace),
        panes: pane_identity_rows(workspace),
    }
}

fn pane_identity_rows_for_controller(controller: RwSignal<AppController>) -> Vec<PaneRow> {
    controller
        .get()
        .model
        .selected_workspace()
        .map(pane_identity_rows)
        .unwrap_or_default()
}

fn pane_identity_rows(workspace: &Workspace) -> Vec<PaneRow> {
    match workspace.layout {
        SplitTree::Leaf(pane_id) => vec![PaneRow { id: pane_id }],
        SplitTree::Split { first, second, .. } => {
            vec![PaneRow { id: first }, PaneRow { id: second }]
        }
    }
}

fn split_direction_for_controller(controller: RwSignal<AppController>) -> PaneStackDirection {
    controller
        .get()
        .model
        .selected_workspace()
        .map(split_direction_for_workspace)
        .unwrap_or(PaneStackDirection::Column)
}

fn split_direction_for_workspace(workspace: &Workspace) -> PaneStackDirection {
    match workspace.layout {
        SplitTree::Leaf(_) => PaneStackDirection::Column,
        SplitTree::Split { axis, .. } => pane_stack_direction(axis),
    }
}

fn pane_stack_direction(axis: SplitAxis) -> PaneStackDirection {
    match axis {
        SplitAxis::Vertical => PaneStackDirection::Row,
        SplitAxis::Horizontal => PaneStackDirection::Column,
    }
}

fn pane_stack_style(s: Style, direction: PaneStackDirection) -> Style {
    let s = match direction {
        PaneStackDirection::Row => s.flex_row(),
        PaneStackDirection::Column => s.flex_col(),
    };
    s.width_full()
        .height_full()
        .min_height(0.0)
        .gap(1.0)
        .background(Color::rgb8(0x25, 0x2a, 0x32))
}

fn pane_selected(controller: RwSignal<AppController>, pane_id: PaneId) -> bool {
    controller
        .get()
        .model
        .selected_workspace()
        .is_ok_and(|workspace| workspace.selected_pane == pane_id)
}

fn surface_tab_identity_rows(
    controller: RwSignal<AppController>,
    pane_id: PaneId,
) -> Vec<SurfaceTabRow> {
    controller
        .get()
        .model
        .selected_workspace()
        .ok()
        .and_then(|workspace| workspace.pane(pane_id))
        .map(|pane| {
            pane.surfaces
                .iter()
                .map(|surface| SurfaceTabRow { id: surface.id })
                .collect()
        })
        .unwrap_or_default()
}

fn surface_tab_state_for_controller(
    controller: RwSignal<AppController>,
    pane_id: PaneId,
    surface_id: SurfaceId,
) -> Option<SurfaceTabState> {
    surface_tab_state(&controller.get().model, pane_id, surface_id)
}

fn surface_tab_state(
    model: &AppModel,
    pane_id: PaneId,
    surface_id: SurfaceId,
) -> Option<SurfaceTabState> {
    let pane = model
        .selected_workspace()
        .ok()
        .and_then(|workspace| workspace.pane(pane_id))?;
    let surface = pane.surface(surface_id)?;
    Some(SurfaceTabState {
        label: surface_tab_label(surface),
        selected: pane.selected_surface == surface_id,
    })
}

fn terminal_content_rows(
    controller: RwSignal<AppController>,
    pane_id: PaneId,
) -> Vec<TerminalContentRow> {
    let controller = controller.get();
    let row = selected_terminal_entry(&controller, pane_id)
        .map(|(surface_id, entry)| TerminalContentRow {
            surface_id,
            entry: Some(entry),
        })
        .unwrap_or_else(|| TerminalContentRow {
            surface_id: SurfaceId(0),
            entry: None,
        });

    vec![row]
}

fn selected_terminal_entry(
    controller: &AppController,
    pane_id: PaneId,
) -> Option<(SurfaceId, TerminalEntry)> {
    let workspace = controller.model.selected_workspace().ok()?;
    let pane = workspace.pane(pane_id)?;
    let surface = pane.surface(pane.selected_surface)?;
    if surface.kind != SurfaceKind::Terminal {
        return None;
    }

    controller
        .terminals
        .entry(surface.id)
        .cloned()
        .map(|entry| (surface.id, entry))
}

fn selected_workspace_title(controller: RwSignal<AppController>) -> String {
    controller
        .get()
        .model
        .selected_workspace()
        .map(|workspace| workspace.title.clone())
        .unwrap_or_else(|_| "Workspace".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::{Error, ErrorKind};
    use std::time::{SystemTime, UNIX_EPOCH};
    use umux_app::session_store::SessionLoadOutcome;
    use umux_app::{AppAction, AppController, SessionStore, SessionStoreError};
    use umux_config::default_shortcuts;
    use umux_core::model::{Pane, SplitTree, Surface, SurfaceKind, Workspace};
    use umux_core::{PaneId, SurfaceId, WorkspaceId};

    #[test]
    fn workspace_row_marks_unread_workspace() {
        let mut workspace = workspace("alpha");
        workspace.unread = true;

        assert_eq!(workspace_row_label(&workspace), "alpha *");
    }

    #[test]
    fn tab_label_marks_unread_surface() {
        let mut surface = surface("Terminal");
        surface.unread = true;

        assert_eq!(surface_tab_label(&surface), "Terminal *");
    }

    #[test]
    fn maps_safe_shell_shortcuts_to_actions() {
        let mut model = AppModel::new("C:/work/alpha");
        let mut workspaces = vec![model.selected_workspace().unwrap().id];
        for index in 2..=9 {
            let workspace = model
                .create_workspace(
                    format!("C:/work/workspace-{index}"),
                    Some(format!("Workspace {index}")),
                )
                .unwrap();
            workspaces.push(workspace);
        }
        let selected_workspace = model.selected_workspace().unwrap().id;
        let selected_surface = model.selected_pane().unwrap().selected_surface;

        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+N"),
            Some(AppAction::NewWorkspace {
                cwd: ".".to_string(),
                title: None,
            })
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+T"),
            Some(AppAction::NewTerminalTab)
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+W"),
            Some(AppAction::CloseSurface(selected_surface))
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+Alt+D"),
            Some(AppAction::SplitPane(SplitAxis::Vertical))
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+Shift+Alt+D"),
            Some(AppAction::SplitPane(SplitAxis::Horizontal))
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+Shift+U"),
            Some(AppAction::JumpLatestUnread)
        );
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+Shift+W"),
            Some(AppAction::CloseWorkspace(selected_workspace))
        );

        for (index, workspace_id) in workspaces.iter().take(8).enumerate() {
            assert_eq!(
                shell_action_for_shortcut(&model, &format!("Ctrl+{}", index + 1)),
                Some(AppAction::SelectWorkspace(*workspace_id))
            );
        }
        assert_eq!(
            shell_action_for_shortcut(&model, "Ctrl+9"),
            Some(AppAction::SelectWorkspace(*workspaces.last().unwrap()))
        );
        assert_eq!(shell_action_for_shortcut(&model, "Ctrl+X"), None);
    }

    #[test]
    fn default_shortcuts_are_classified_for_mvp_runtime() {
        let shell_actions = [
            "new_workspace",
            "jump_workspace_1_8",
            "jump_last_workspace",
            "close_workspace",
            "new_surface",
            "close_surface",
            "split_right",
            "split_down",
            "jump_latest_unread",
        ];
        let terminal_actions = ["copy", "paste"];
        let deferred_actions = [
            "toggle_sidebar",
            "open_browser_split",
            "focus_address_bar",
            "show_notifications",
            "clear_scrollback",
            "settings",
        ];

        for action in shell_actions {
            assert_eq!(
                shortcut_disposition_for_action(action),
                ShortcutDisposition::Shell
            );
        }
        for action in terminal_actions {
            assert_eq!(
                shortcut_disposition_for_action(action),
                ShortcutDisposition::Terminal
            );
        }
        for action in deferred_actions {
            assert_eq!(
                shortcut_disposition_for_action(action),
                ShortcutDisposition::Deferred
            );
        }
        assert_eq!(
            shortcut_disposition_for_action("quit"),
            ShortcutDisposition::Unbound
        );

        for binding in default_shortcuts() {
            if binding.action == "quit" {
                continue;
            }
            assert_ne!(
                shortcut_disposition_for_action(&binding.action),
                ShortcutDisposition::Unbound,
                "{} should be classified",
                binding.action
            );
        }
    }

    #[test]
    fn deferred_shortcuts_are_logged_but_not_handled_by_shell() {
        let model = AppModel::new("C:/work/alpha");

        assert_eq!(shell_action_for_shortcut(&model, "Ctrl+Shift+L"), None);
        assert_eq!(
            deferred_shortcut_action_for_chord("Ctrl+Shift+L").as_deref(),
            Some("open_browser_split")
        );
        assert_eq!(shell_action_for_shortcut(&model, "Ctrl+Shift+C"), None);
        assert_eq!(deferred_shortcut_action_for_chord("Ctrl+Shift+C"), None);
    }

    #[test]
    fn dispatch_action_saves_successful_actions() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let store = temp_session_store("dispatch-save");

        dispatch_action(
            &mut controller,
            &store,
            AppAction::NewWorkspace {
                cwd: "C:/work/beta".to_string(),
                title: Some("Beta".to_string()),
            },
        );

        let loaded = store.load_model().unwrap().unwrap();
        assert_eq!(loaded.selected_workspace().unwrap().title, "Beta");
    }

    #[test]
    fn terminal_notifications_update_controller_model_and_saved_session() {
        let mut controller = AppController::new(AppModel::new("C:/work/alpha")).unwrap();
        let surface_id = controller.model.selected_pane().unwrap().selected_surface;
        let store = temp_session_store("terminal-notification-save");
        let mut emulator = umux_terminal::TerminalEmulator::new(20, 3, 100);
        let notifications = emulator.feed_bytes(b"\x1b]9;Build done\x07");

        apply_terminal_notification_event(
            &mut controller,
            &store,
            TerminalNotificationEvent {
                surface_id,
                notifications,
            },
        );

        let workspace = controller.model.selected_workspace().unwrap();
        assert!(workspace.unread);
        assert_eq!(workspace.latest_notification.as_deref(), Some("Build done"));
        assert_eq!(
            controller
                .model
                .latest_unread_target
                .as_ref()
                .map(|target| target.surface_id),
            Some(surface_id)
        );
        let loaded = store.load_model().unwrap().unwrap();
        assert!(loaded.selected_workspace().unwrap().unread);
        assert_eq!(
            loaded
                .latest_unread_target
                .as_ref()
                .map(|target| target.surface_id),
            Some(surface_id)
        );
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
            decision.warning.map(|warning| warning.message),
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
        assert_eq!(
            decision.warning.map(|warning| warning.message),
            Some(SESSION_READ_WARNING.to_string())
        );
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
    fn startup_warning_banner_message_uses_short_user_facing_text() {
        let warning = StartupWarning {
            message: RESTORE_CONTROLLER_WARNING.to_string(),
        };

        assert_eq!(
            startup_warning_banner_message(&warning),
            "Previous session could not be restored. Opened a fresh workspace."
        );
    }

    #[test]
    fn pane_layout_preserves_split_axis() {
        let mut vertical = AppModel::new("C:/work/alpha");
        vertical.split_selected_pane(SplitAxis::Vertical).unwrap();
        let vertical_layout = pane_layout_for_workspace(vertical.selected_workspace().unwrap());

        let mut horizontal = AppModel::new("C:/work/alpha");
        horizontal
            .split_selected_pane(SplitAxis::Horizontal)
            .unwrap();
        let horizontal_layout = pane_layout_for_workspace(horizontal.selected_workspace().unwrap());

        assert_eq!(vertical_layout.direction, PaneStackDirection::Row);
        assert_eq!(horizontal_layout.direction, PaneStackDirection::Column);
        assert_eq!(vertical_layout.panes.len(), 2);
        assert_eq!(horizontal_layout.panes.len(), 2);
    }

    #[test]
    fn dynamic_row_keys_ignore_label_and_selected_state() {
        let workspace_before = WorkspaceRow {
            id: WorkspaceId(10),
        };
        let workspace_after = WorkspaceRow {
            id: WorkspaceId(10),
        };
        let tab_before = SurfaceTabRow { id: SurfaceId(20) };
        let tab_after = SurfaceTabRow { id: SurfaceId(20) };

        assert_eq!(
            workspace_row_key(&workspace_before),
            workspace_row_key(&workspace_after)
        );
        assert_eq!(surface_tab_key(&tab_before), surface_tab_key(&tab_after));
    }

    #[test]
    fn workspace_row_state_recomputes_by_id_after_model_changes() {
        let mut model = AppModel::new("C:/work/alpha");
        let alpha = model.selected_workspace().unwrap().id;
        let beta = model
            .create_workspace("C:/work/beta", Some("Beta".to_string()))
            .unwrap();
        let beta_surface = model.selected_pane().unwrap().selected_surface;

        let before = workspace_row_state(&model, alpha).unwrap();
        model
            .mark_surface_unread(beta_surface, "done".to_string())
            .unwrap();
        let alpha_after = workspace_row_state(&model, alpha).unwrap();
        let beta_after = workspace_row_state(&model, beta).unwrap();

        assert_eq!(before.label, "alpha");
        assert!(!before.selected);
        assert_eq!(alpha_after.label, "alpha");
        assert!(!alpha_after.selected);
        assert_eq!(beta_after.label, "Beta *");
        assert!(beta_after.selected);
    }

    #[test]
    fn surface_tab_state_recomputes_by_id_after_model_changes() {
        let mut model = AppModel::new("C:/work/alpha");
        let pane_id = model.selected_pane().unwrap().id;
        let first = model.selected_pane().unwrap().selected_surface;
        let second = model.open_terminal_surface().unwrap();

        model
            .mark_surface_unread(first, "first done".to_string())
            .unwrap();
        let first_state = surface_tab_state(&model, pane_id, first).unwrap();
        let second_state = surface_tab_state(&model, pane_id, second).unwrap();

        assert_eq!(first_state.label, "Terminal *");
        assert!(!first_state.selected);
        assert_eq!(second_state.label, "Terminal");
        assert!(second_state.selected);
    }

    #[test]
    fn pane_identity_rows_ignore_selected_state() {
        let mut model = AppModel::new("C:/work/alpha");
        let first = model.selected_pane().unwrap().id;
        let second = model.split_selected_pane(SplitAxis::Vertical).unwrap();

        let before = pane_identity_rows(model.selected_workspace().unwrap());
        model.select_pane(first).unwrap();
        let after = pane_identity_rows(model.selected_workspace().unwrap());

        assert_eq!(before, vec![PaneRow { id: first }, PaneRow { id: second }]);
        assert_eq!(before, after);
    }

    fn workspace(title: &str) -> Workspace {
        Workspace {
            id: WorkspaceId(1),
            title: title.to_string(),
            cwd: "C:/work/alpha".to_string(),
            panes: vec![Pane {
                id: PaneId(2),
                cwd: "C:/work/alpha".to_string(),
                surfaces: vec![surface("Terminal")],
                selected_surface: SurfaceId(3),
            }],
            selected_pane: PaneId(2),
            layout: SplitTree::Leaf(PaneId(2)),
            unread: false,
            latest_notification: None,
        }
    }

    fn surface(title: &str) -> Surface {
        Surface {
            id: SurfaceId(3),
            kind: SurfaceKind::Terminal,
            title: title.to_string(),
            unread: false,
            unread_message: None,
            unread_sequence: None,
        }
    }

    fn temp_session_store(name: &str) -> SessionStore {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut path = SessionStore::default_path();
        path.pop();
        path.push("umux-ui-shell-tests");
        path.push(format!("{name}-{nanos}-{}", std::process::id()));
        fs::remove_dir_all(path.as_std_path()).ok();
        SessionStore::new(path.join("session.json"))
    }
}

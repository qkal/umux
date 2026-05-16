// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::sync::{Arc, Mutex};

use floem::event::{Event, EventListener, EventPropagation};
use floem::ext_event::create_signal_from_channel;
use floem::keyboard::{Key, NamedKey};
use floem::prelude::*;
use floem::reactive::create_effect;
use floem::style::Style;
use umux_app::{AppAction, AppController, SessionStore, TerminalEntry};
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

fn app_view() -> impl IntoView {
    let store = SessionStore::new(SessionStore::default_path());
    let model = store.load_model().ok().flatten().unwrap_or_else(seed_model);
    let controller = AppController::from_restored_model(model).unwrap_or_else(|_| {
        AppController::new(seed_model()).expect("seed model should create an app controller")
    });

    shell_view(controller, store)
}

fn shell_view(controller: AppController, store: SessionStore) -> impl IntoView {
    let shared_model = Arc::new(Mutex::new(controller.model.clone()));
    let controller = create_rw_signal(controller);
    let store = Arc::new(store);
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

    app_shell(controller, store, shared_model, notification_tx)
}

fn dispatch_action(controller: &mut AppController, store: &SessionStore, action: AppAction) {
    if controller.apply(action).is_ok() {
        let _ = store.save_model(&controller.model);
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
        let _ = store.save_model(&controller.model);
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
        let _ = store.save_model(&controller.model);
    }

    changed
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
) -> impl IntoView {
    let shortcut_store = store.clone();
    let shortcut_shared_model = shared_model.clone();

    v_stack((
        top_bar(controller, store.clone(), shared_model.clone()),
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
        let Some(action) = action_for_shortcut(&chord).map(runtime_shortcut_action) else {
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

fn action_for_shortcut(chord: &str) -> Option<AppAction> {
    match chord {
        "Ctrl+N" => Some(AppAction::NewWorkspace {
            cwd: ".".to_string(),
            title: None,
        }),
        "Ctrl+T" => Some(AppAction::NewTerminalTab),
        "Ctrl+Alt+D" => Some(AppAction::SplitPane(SplitAxis::Vertical)),
        "Ctrl+Shift+U" => Some(AppAction::JumpLatestUnread),
        _ => None,
    }
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
                    store.clone(),
                    shared_model.clone(),
                    AppAction::NewWorkspace { cwd, title: None },
                );
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
    h_stack((
        label(move || selected_workspace_title(controller))
            .style(|s| s.color(TEXT).font_size(13.0).font_bold().text_ellipsis()),
        h_stack((
            button(label(|| "+ tab"))
                .action({
                    let store = store.clone();
                    let shared_model = shared_model.clone();
                    move || {
                        dispatch_shell_action(
                            controller,
                            store.clone(),
                            shared_model.clone(),
                            AppAction::NewTerminalTab,
                        );
                    }
                })
                .style(compact_button_style),
            button(label(|| "split"))
                .action(move || {
                    dispatch_shell_action(
                        controller,
                        store.clone(),
                        shared_model.clone(),
                        AppAction::SplitPane(SplitAxis::Vertical),
                    );
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
    use std::time::{SystemTime, UNIX_EPOCH};
    use umux_app::{AppAction, AppController, SessionStore};
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
    fn maps_shell_shortcuts_to_actions() {
        assert_eq!(
            action_for_shortcut("Ctrl+N"),
            Some(AppAction::NewWorkspace {
                cwd: ".".to_string(),
                title: None,
            })
        );
        assert_eq!(
            action_for_shortcut("Ctrl+T"),
            Some(AppAction::NewTerminalTab)
        );
        assert_eq!(
            action_for_shortcut("Ctrl+Alt+D"),
            Some(AppAction::SplitPane(SplitAxis::Vertical))
        );
        assert_eq!(
            action_for_shortcut("Ctrl+Shift+U"),
            Some(AppAction::JumpLatestUnread)
        );
        assert_eq!(action_for_shortcut("Ctrl+X"), None);
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

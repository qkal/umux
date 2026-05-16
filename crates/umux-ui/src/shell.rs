// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::sync::{Arc, Mutex};

use floem::prelude::*;
use floem::style::Style;
use umux_app::{AppAction, AppController, SessionStore, TerminalEntry};
use umux_core::AppModel;
use umux_core::model::{SplitTree, Workspace};
use umux_core::{PaneId, SplitAxis, SurfaceId, SurfaceKind, WorkspaceId};

use crate::terminal_view::{SharedAppModel, terminal_view_for_entry};
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
    let cwd = env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    AppModel::new(cwd)
}

fn app_view() -> impl IntoView {
    let store = SessionStore::new(SessionStore::default_path());
    let model = store
        .load_model()
        .ok()
        .flatten()
        .unwrap_or_else(seed_model);
    let controller = AppController::from_restored_model(model).unwrap_or_else(|_| {
        AppController::new(seed_model()).expect("seed model should create an app controller")
    });

    shell_view(controller, store)
}

fn shell_view(controller: AppController, store: SessionStore) -> impl IntoView {
    let shared_model = Arc::new(Mutex::new(controller.model.clone()));
    let controller = create_rw_signal(controller);
    app_shell(controller, Arc::new(store), shared_model)
}

fn dispatch_action(controller: &mut AppController, store: &SessionStore, action: AppAction) {
    if controller.apply(action).is_ok() {
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
        if let Ok(mut model) = shared_model.lock() {
            *model = controller.model.clone();
        }
    });
}

fn app_shell(
    controller: RwSignal<AppController>,
    store: Arc<SessionStore>,
    shared_model: SharedAppModel,
) -> impl IntoView {
    v_stack((
        top_bar(controller, store.clone(), shared_model.clone()),
        h_stack((
            sidebar(controller, store.clone(), shared_model.clone()),
            work_area(controller, store, shared_model),
        ))
        .style(|s| s.flex().width_full().height_full().min_width(0.0)),
    ))
    .style(|s| s.size_full().background(BACKGROUND).color(TEXT))
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
            |row| (row.id, row.label.clone(), row.selected),
            move |row| {
                workspace_row_button(
                    row,
                    controller,
                    row_store.clone(),
                    row_shared_model.clone(),
                )
            },
        )
        .style(|s| s.width_full().flex_col().gap(4.0)),
        button(label(|| "+ ws"))
            .action(move || {
                let cwd = env::current_dir()
                    .ok()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| ".".to_string());
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
) -> impl IntoView {
    v_stack((
        workspace_controls(controller, store.clone(), shared_model.clone()),
        dyn_stack(
            move || pane_rows(controller),
            |pane| (pane.id, pane.selected),
            move |pane| pane_view(pane, controller, store.clone(), shared_model.clone()),
        )
        .style(|s| {
            s.width_full()
                .height_full()
                .min_height(0.0)
                .flex_col()
                .gap(1.0)
                .background(Color::rgb8(0x25, 0x2a, 0x32))
        }),
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
) -> impl IntoView {
    let pane_id = pane.id;
    let tab_shared_model = shared_model.clone();

    v_stack((
        dyn_stack(
            move || surface_tab_rows(controller, pane_id),
            |tab| (tab.id, tab.label.clone(), tab.selected),
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
            |content| (content.surface_id, content.available),
            move |content| terminal_content_view(content, shared_model.clone()),
        )
        .style(|s| s.width_full().height_full().min_height(0.0)),
    ))
    .style(move |s| {
        let border = if pane.selected {
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
) -> impl IntoView {
    match content.entry {
        Some(entry) => terminal_view_for_entry(Arc::new(entry), Some(shared_model))
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
    let action = AppAction::SelectWorkspace(row.id);
    button(label(move || row.label.clone()).style(|s| s.text_ellipsis()))
        .action(move || {
            dispatch_shell_action(controller, store.clone(), shared_model.clone(), action.clone());
        })
        .style(move |s| {
            let background = if row.selected {
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
    button(label(move || tab.label.clone()).style(|s| s.text_ellipsis()))
        .action(move || {
            dispatch_shell_action(
                controller,
                store.clone(),
                shared_model.clone(),
                AppAction::SelectPane(pane_id),
            );
            dispatch_shell_action(
                controller,
                store.clone(),
                shared_model.clone(),
                AppAction::SelectSurface(surface_id),
            );
        })
        .style(move |s| {
            let background = if tab.selected {
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
    label: String,
    selected: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PaneRow {
    id: PaneId,
    selected: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SurfaceTabRow {
    id: SurfaceId,
    label: String,
    selected: bool,
}

#[derive(Clone)]
struct TerminalContentRow {
    surface_id: SurfaceId,
    available: bool,
    entry: Option<TerminalEntry>,
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
                .map(|workspace| WorkspaceRow {
                    id: workspace.id,
                    label: workspace_row_label(workspace),
                    selected: workspace.id == window.selected_workspace,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn pane_rows(controller: RwSignal<AppController>) -> Vec<PaneRow> {
    let controller = controller.get();
    let Ok(workspace) = controller.model.selected_workspace() else {
        return Vec::new();
    };

    match workspace.layout {
        SplitTree::Leaf(pane_id) => vec![pane_row(workspace, pane_id)],
        SplitTree::Split { first, second, .. } => {
            vec![pane_row(workspace, first), pane_row(workspace, second)]
        }
    }
}

fn pane_row(workspace: &Workspace, pane_id: PaneId) -> PaneRow {
    PaneRow {
        id: pane_id,
        selected: workspace.selected_pane == pane_id,
    }
}

fn surface_tab_rows(
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
                .map(|surface| SurfaceTabRow {
                    id: surface.id,
                    label: surface_tab_label(surface),
                    selected: pane.selected_surface == surface.id,
                })
                .collect()
        })
        .unwrap_or_default()
}

fn terminal_content_rows(
    controller: RwSignal<AppController>,
    pane_id: PaneId,
) -> Vec<TerminalContentRow> {
    let controller = controller.get();
    let row = selected_terminal_entry(&controller, pane_id)
        .map(|(surface_id, entry)| TerminalContentRow {
            surface_id,
            available: true,
            entry: Some(entry),
        })
        .unwrap_or_else(|| TerminalContentRow {
            surface_id: SurfaceId(0),
            available: false,
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

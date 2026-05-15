// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;

use floem::prelude::*;
use umux_core::AppModel;

use crate::terminal_view::terminal_view;
use crate::theme::{SIDEBAR_WIDTH, SURFACE_TAB_HEIGHT, TOP_BAR_HEIGHT};

const BACKGROUND: Color = Color::rgb8(0x11, 0x13, 0x16);
const PANEL: Color = Color::rgb8(0x18, 0x1b, 0x20);
const TEXT: Color = Color::rgb8(0xe7, 0xea, 0xf0);
const MUTED_TEXT: Color = Color::rgb8(0x9b, 0xa3, 0xaf);
const UNREAD_BLUE: Color = Color::rgb8(0x2f, 0x80, 0xff);

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
    shell_view(seed_model())
}

fn shell_view(model: AppModel) -> impl IntoView {
    let workspace = model.selected_workspace().ok();
    let workspace_title = workspace
        .map(|workspace| workspace.title.clone())
        .unwrap_or_else(|| "Workspace".to_string());
    let surface_count = model
        .selected_pane()
        .map(|pane| pane.surfaces.len())
        .unwrap_or_default();

    v_stack((
        top_bar(workspace_title.clone()),
        h_stack((sidebar(workspace_title), work_area(surface_count)))
            .style(|s| s.flex().width_full().height_full()),
    ))
    .style(|s| s.size_full().background(BACKGROUND).color(TEXT))
}

fn top_bar(workspace_title: String) -> impl IntoView {
    h_stack((
        label(|| "umux"),
        label(move || workspace_title.clone()).style(|s| s.color(MUTED_TEXT)),
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

fn sidebar(workspace_title: String) -> impl IntoView {
    v_stack((
        label(move || workspace_title.clone()).style(|s| s.color(TEXT).font_size(14.0).font_bold()),
        label(|| "Terminal - Browser - Notifications")
            .style(|s| s.color(MUTED_TEXT).font_size(12.0)),
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

fn work_area(surface_count: usize) -> impl IntoView {
    let surface_count_label = format!("{surface_count} surface");

    v_stack((
        h_stack((
            label(|| "Terminal surface").style(|s| s.color(TEXT).font_size(13.0)),
            label(move || surface_count_label.clone())
                .style(|s| s.color(MUTED_TEXT).font_size(12.0)),
        ))
        .style(|s| {
            s.height(SURFACE_TAB_HEIGHT)
                .width_full()
                .items_center()
                .justify_between()
                .padding_horiz(12.0)
                .background(Color::rgb8(0x14, 0x17, 0x1b))
                .border_bottom(1.0)
                .border_color(Color::rgb8(0x25, 0x2a, 0x32))
        }),
        terminal_view().style(|s| {
            s.width_full()
                .height_full()
                .padding(16.0)
                .gap(8.0)
                .background(BACKGROUND)
                .border_left(3.0)
                .border_color(UNREAD_BLUE)
        }),
    ))
    .style(|s| s.width_full().height_full().background(BACKGROUND))
}

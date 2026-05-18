// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Div, MouseButton, div, prelude::*, px};
use umux_core::SurfaceId;
use umux_ui_kit::{BORDER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE};

use crate::view_model::SurfaceTab;

pub fn surface_tabs(
    tabs: Vec<SurfaceTab>,
    on_select: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_close: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_new: impl Fn(&mut App) + Clone + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(34.0))
        .bg(PANEL)
        .border_b_1()
        .border_color(BORDER)
        .children(
            tabs.into_iter()
                .map(move |tab| surface_tab(tab, on_select.clone(), on_close.clone())),
        )
        .child(new_tab_control(on_new))
}

fn surface_tab(
    tab: SurfaceTab,
    on_select: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_close: impl Fn(SurfaceId, &mut App) + Clone + 'static,
) -> Div {
    let id = tab.id;

    div()
        .flex()
        .items_center()
        .h_full()
        .px(px(12.0))
        .text_size(px(12.0))
        .text_color(if tab.selected { TEXT } else { MUTED_TEXT })
        .when(tab.selected, |tab| tab.bg(BORDER))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(id, cx))
        .child(tab.label)
        .when(tab.unread, |tab| {
            tab.child(
                div()
                    .ml(px(6.0))
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(UNREAD_BLUE),
            )
        })
        .child(close_tab_control(id, on_close))
}

fn close_tab_control(
    surface_id: SurfaceId,
    on_close: impl Fn(SurfaceId, &mut App) + Clone + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .ml(px(8.0))
        .w(px(18.0))
        .h(px(18.0))
        .text_size(px(12.0))
        .text_color(MUTED_TEXT)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            on_close(surface_id, cx);
        })
        .child("x")
}

fn new_tab_control(on_new: impl Fn(&mut App) + Clone + 'static) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h_full()
        .min_w(px(34.0))
        .px(px(8.0))
        .text_size(px(14.0))
        .text_color(MUTED_TEXT)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_new(cx))
        .child("+")
}

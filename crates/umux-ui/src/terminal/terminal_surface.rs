// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::IntoElement;
use umux_app::AppController;
use umux_core::SurfaceId;

use crate::terminal::{bridge::state_from_entry, terminal_element};

pub fn terminal_surface(controller: &AppController, surface_id: SurfaceId) -> impl IntoElement {
    let state = state_from_entry(controller.terminals.entry(surface_id));

    terminal_element(state.status, state.snapshot)
}

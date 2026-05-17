// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, AppContext};
use umux_app::SessionStore;

use crate::startup::startup_state_from_store;
use crate::workspace::UmuxWorkspace;

pub fn run() {
    crate::diagnostics::init_diagnostics();
    gpui_platform::application().run(|cx: &mut App| {
        let store = SessionStore::new(SessionStore::default_path());
        let startup = startup_state_from_store(&store);
        cx.open_window(Default::default(), |_, cx| {
            cx.new(|_| UmuxWorkspace::new(startup, store))
        })
        .expect("open umux GPUI window");
        cx.activate(true);
    });
}

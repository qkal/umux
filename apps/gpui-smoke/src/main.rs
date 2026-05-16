// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Context, IntoElement, Render, Window, div, prelude::*, rgb};

struct SmokeWindow;

impl Render for SmokeWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x111316))
            .text_color(rgb(0xe7eaf0))
            .child("umux GPUI smoke")
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        cx.open_window(Default::default(), |_, cx| cx.new(|_| SmokeWindow))
            .expect("open GPUI smoke window");
        cx.activate(true);
    });
}

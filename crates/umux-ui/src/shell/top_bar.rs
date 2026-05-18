// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Div, MouseButton, div, prelude::*, px};
use umux_ui_kit::{
    ACTIVE, BORDER, DIM_TEXT, ELEVATED, HOVER, MUTED_TEXT, PANEL, TEXT,
};

pub fn top_bar(
    title: String,
    on_new_terminal_tab: impl Fn(&mut App) + Clone + 'static,
    on_split_right: impl Fn(&mut App) + Clone + 'static,
    on_split_down: impl Fn(&mut App) + Clone + 'static,
    on_jump_latest_unread: impl Fn(&mut App) + Clone + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .w_full()
        .h(px(42.0))
        .px(px(12.0))
        .bg(PANEL)
        .border_b_1()
        .border_color(BORDER)
        .text_size(px(12.0))
        .child(
            div()
                .flex()
                .items_center()
                .min_w(px(0.0))
                .child(div().font_weight(gpui::FontWeight::BOLD).text_color(TEXT).child("umux"))
                .child(
                    div()
                        .ml(px(12.0))
                        .text_color(DIM_TEXT)
                        .child("workspace:"),
                )
                .child(
                    div()
                        .ml(px(6.0))
                        .min_w(px(0.0))
                        .text_color(MUTED_TEXT)
                        .truncate()
                        .child(title),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .children([
                    top_bar_action("+", on_new_terminal_tab).into_any_element(),
                    top_bar_action("split >", on_split_right).into_any_element(),
                    top_bar_action("split v", on_split_down).into_any_element(),
                    top_bar_action("!", on_jump_latest_unread).into_any_element(),
                ]),
        )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn top_bar_action_labels() -> [&'static str; 4] {
    ["+", "split >", "split v", "!"]
}

fn top_bar_action(
    label: &'static str,
    on_click: impl Fn(&mut App) + Clone + 'static,
) -> impl IntoElement {
    div()
        .id(("top-bar-action", top_bar_action_id(label)))
        .flex()
        .items_center()
        .justify_center()
        .h(px(24.0))
        .min_w(px(28.0))
        .ml(px(6.0))
        .px(px(8.0))
        .border_1()
        .border_color(BORDER)
        .bg(ELEVATED)
        .text_color(MUTED_TEXT)
        .text_size(px(11.0))
        .cursor_pointer()
        .hover(|style| style.bg(HOVER).text_color(TEXT))
        .active(|style| style.bg(ACTIVE))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_click(cx))
        .child(label)
}

fn top_bar_action_id(label: &str) -> usize {
    top_bar_action_labels()
        .iter()
        .position(|candidate| *candidate == label)
        .expect("top-bar action label should have a stable id")
}

#[cfg(test)]
mod tests {
    use super::top_bar_action_labels;

    #[test]
    fn top_bar_exposes_shell_action_labels() {
        assert_eq!(top_bar_action_labels(), ["+", "split >", "split v", "!"]);
    }
}

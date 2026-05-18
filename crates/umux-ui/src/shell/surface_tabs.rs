// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::{App, Div, Keystroke, MouseButton, div, prelude::*, px};
use umux_core::SurfaceId;
use umux_ui_kit::{BORDER, MUTED_TEXT, PANEL, TEXT, UNREAD_BLUE};

use crate::view_model::SurfaceTab;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenameEdit {
    Insert(char),
    Backspace,
    Commit,
    Cancel,
}

#[allow(clippy::too_many_arguments)]
pub fn surface_tabs(
    tabs: Vec<SurfaceTab>,
    renaming_surface: Option<SurfaceId>,
    rename_buffer: String,
    on_select: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_close: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_new: impl Fn(&mut App) + Clone + 'static,
    on_start_rename: impl Fn(SurfaceId, String, &mut App) + Clone + 'static,
    on_rename_edit: impl Fn(SurfaceId, RenameEdit, &mut App) + Clone + 'static,
) -> Div {
    div()
        .flex()
        .items_center()
        .w_full()
        .h(px(34.0))
        .bg(PANEL)
        .border_b_1()
        .border_color(BORDER)
        .children(tabs.into_iter().map(move |tab| {
            surface_tab(
                tab,
                renaming_surface,
                rename_buffer.clone(),
                on_select.clone(),
                on_close.clone(),
                on_start_rename.clone(),
                on_rename_edit.clone(),
            )
        }))
        .child(new_tab_control(on_new))
}

fn surface_tab(
    tab: SurfaceTab,
    renaming_surface: Option<SurfaceId>,
    rename_buffer: String,
    on_select: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_close: impl Fn(SurfaceId, &mut App) + Clone + 'static,
    on_start_rename: impl Fn(SurfaceId, String, &mut App) + Clone + 'static,
    on_rename_edit: impl Fn(SurfaceId, RenameEdit, &mut App) + Clone + 'static,
) -> Div {
    let id = tab.id;
    let title = tab.title;
    let label = tab.label;
    let selected = tab.selected;
    let unread = tab.unread;
    let is_renaming = renaming_surface == Some(id);

    div()
        .flex()
        .items_center()
        .h_full()
        .px(px(12.0))
        .text_size(px(12.0))
        .text_color(if selected { TEXT } else { MUTED_TEXT })
        .when(selected, |tab| tab.bg(BORDER))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| on_select(id, cx))
        .child(if is_renaming {
            rename_editor(id, rename_buffer, on_rename_edit).into_any_element()
        } else {
            div().child(label).truncate().into_any_element()
        })
        .when(unread, |tab| {
            tab.child(
                div()
                    .ml(px(6.0))
                    .w(px(6.0))
                    .h(px(6.0))
                    .rounded_full()
                    .bg(UNREAD_BLUE),
            )
        })
        .when(selected && !is_renaming, |tab| {
            tab.child(rename_tab_control(id, title, on_start_rename))
        })
        .when(!is_renaming, |tab| {
            tab.child(close_tab_control(id, on_close))
        })
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

fn rename_tab_control(
    surface_id: SurfaceId,
    title: String,
    on_start_rename: impl Fn(SurfaceId, String, &mut App) + Clone + 'static,
) -> impl IntoElement {
    div()
        .id(("surface-rename", surface_id.0))
        .focusable()
        .flex()
        .items_center()
        .justify_center()
        .ml(px(8.0))
        .h(px(18.0))
        .px(px(6.0))
        .border_1()
        .border_color(BORDER)
        .text_size(px(10.0))
        .text_color(MUTED_TEXT)
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            cx.stop_propagation();
            on_start_rename(surface_id, title.clone(), cx);
        })
        .child("rename")
}

fn rename_editor(
    surface_id: SurfaceId,
    buffer: String,
    on_rename_edit: impl Fn(SurfaceId, RenameEdit, &mut App) + Clone + 'static,
) -> impl IntoElement {
    div()
        .id(("surface-rename", surface_id.0))
        .focusable()
        .key_context("SurfaceRename")
        .flex()
        .items_center()
        .w(px(150.0))
        .h(px(22.0))
        .px(px(6.0))
        .border_1()
        .border_color(TEXT)
        .text_color(TEXT)
        .text_size(px(12.0))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_key_down(move |event, _window, cx| {
            if let Some(edit) = rename_edit_from_keystroke(&event.keystroke) {
                cx.stop_propagation();
                on_rename_edit(surface_id, edit, cx);
            }
        })
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .truncate()
                .child(if buffer.is_empty() {
                    " ".to_string()
                } else {
                    buffer
                }),
        )
        .child(div().ml(px(2.0)).w(px(1.0)).h(px(14.0)).bg(TEXT))
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

fn rename_edit_from_keystroke(keystroke: &Keystroke) -> Option<RenameEdit> {
    match keystroke.key.as_str() {
        "enter" => Some(RenameEdit::Commit),
        "escape" => Some(RenameEdit::Cancel),
        "backspace" => Some(RenameEdit::Backspace),
        "space" if accepts_rename_text(keystroke) => Some(RenameEdit::Insert(' ')),
        _ => typed_rename_character(keystroke).map(RenameEdit::Insert),
    }
}

fn typed_rename_character(keystroke: &Keystroke) -> Option<char> {
    if !accepts_rename_text(keystroke) {
        return None;
    }

    keystroke.key_char.as_deref().and_then(single_character)
}

fn accepts_rename_text(keystroke: &Keystroke) -> bool {
    !keystroke.modifiers.control
        && !keystroke.modifiers.alt
        && !keystroke.modifiers.platform
        && !keystroke.modifiers.function
}

fn single_character(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    chars.next().is_none().then_some(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(key: &str, key_char: Option<&str>) -> Keystroke {
        Keystroke {
            key: key.to_string(),
            key_char: key_char.map(ToString::to_string),
            ..Keystroke::default()
        }
    }

    #[test]
    fn rename_key_edits_text_and_control_keys() {
        assert_eq!(
            rename_edit_from_keystroke(&key("a", Some("a"))),
            Some(RenameEdit::Insert('a'))
        );
        assert_eq!(
            rename_edit_from_keystroke(&key("space", None)),
            Some(RenameEdit::Insert(' '))
        );
        assert_eq!(
            rename_edit_from_keystroke(&key("backspace", None)),
            Some(RenameEdit::Backspace)
        );
        assert_eq!(
            rename_edit_from_keystroke(&key("enter", None)),
            Some(RenameEdit::Commit)
        );
        assert_eq!(
            rename_edit_from_keystroke(&key("escape", None)),
            Some(RenameEdit::Cancel)
        );
    }

    #[test]
    fn rename_key_ignores_modified_shortcuts_and_multi_char_text() {
        let mut shortcut = key("a", Some("a"));
        shortcut.modifiers.control = true;
        assert_eq!(rename_edit_from_keystroke(&shortcut), None);
        assert_eq!(rename_edit_from_keystroke(&key("ime", Some("ab"))), None);
    }
}

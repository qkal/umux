// SPDX-License-Identifier: GPL-3.0-or-later

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use gpui::{Bounds, ClipboardItem, IntoElement, Keystroke, Pixels, div, prelude::*, px};
use umux_app::{AppController, TerminalEntryHandle};
use umux_core::SurfaceId;
use umux_terminal::{
    TerminalInputRoute, TerminalInputRouter, TerminalKey, TerminalKeyEvent, TerminalMetrics,
    TerminalRendererSnapshot,
};

use crate::terminal::{bridge::state_from_entry, terminal_element};

#[derive(Clone, Default)]
pub struct TerminalSurfaceState {
    last_grid_sizes: Rc<RefCell<HashMap<SurfaceId, (u16, u16)>>>,
}

impl TerminalSurfaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub(crate) fn grid_size_changed(
        &self,
        surface_id: SurfaceId,
        cols: u16,
        rows: u16,
    ) -> Option<(u16, u16)> {
        let mut last_grid_sizes = self.last_grid_sizes.borrow_mut();
        let next = (cols, rows);
        if last_grid_sizes.get(&surface_id) == Some(&next) {
            return None;
        }

        last_grid_sizes.insert(surface_id, next);
        Some(next)
    }
}

pub fn terminal_surface(
    controller: &AppController,
    surface_id: SurfaceId,
    surface_state: &TerminalSurfaceState,
) -> impl IntoElement {
    let entry = controller.terminals.entry(surface_id);
    let state = state_from_entry(entry);
    let input_handle = entry.map(|entry| entry.weak_handle());
    let resize_handle = input_handle.clone();
    let resize_state = surface_state.clone();
    let selection_text = selected_text_for_clipboard(state.snapshot.as_ref());
    let selection_present = selection_text.is_some();

    div()
        .id(("terminal-surface", surface_id.0))
        .focusable()
        .flex()
        .flex_1()
        .min_w(px(0.0))
        .min_h(px(0.0))
        .size_full()
        .on_key_down(move |event, _window, cx| {
            match route_gpui_keystroke(&event.keystroke, selection_present) {
                TerminalInputRoute::WriteBytes(bytes) => {
                    if let Some(handle) = input_handle.as_ref() {
                        handle.send_input(bytes);
                    }
                    cx.stop_propagation();
                }
                TerminalInputRoute::CopySelection => {
                    if let Some(text) = selection_text.as_ref() {
                        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
                    }
                    cx.stop_propagation();
                }
                TerminalInputRoute::PasteClipboard => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text())
                        && let Some(handle) = input_handle.as_ref()
                    {
                        handle.send_input(text);
                    }
                    cx.stop_propagation();
                }
                TerminalInputRoute::Ignore => {}
            }
        })
        .child(
            terminal_element(state.status, state.snapshot).on_bounds(move |bounds| {
                resize_terminal_entry(surface_id, bounds, &resize_state, resize_handle.as_ref());
            }),
        )
}

fn selected_text_for_clipboard(snapshot: Option<&TerminalRendererSnapshot>) -> Option<String> {
    snapshot
        .and_then(TerminalRendererSnapshot::selected_text)
        .filter(|text| !text.is_empty())
}

pub fn route_gpui_keystroke(keystroke: &Keystroke, selection_present: bool) -> TerminalInputRoute {
    let Some(key) = terminal_key_from_keystroke(keystroke) else {
        return TerminalInputRoute::Ignore;
    };

    TerminalInputRouter::route_key(TerminalKeyEvent {
        key,
        ctrl: keystroke.modifiers.control,
        shift: keystroke.modifiers.shift,
        alt: keystroke.modifiers.alt,
        selection_present,
    })
}

pub fn terminal_key_from_keystroke(keystroke: &Keystroke) -> Option<TerminalKey> {
    match keystroke.key.as_str() {
        "enter" => Some(TerminalKey::Enter),
        "backspace" => Some(TerminalKey::Backspace),
        "escape" => Some(TerminalKey::Escape),
        "tab" => Some(TerminalKey::Tab),
        "space" => Some(TerminalKey::Character(' ')),
        _ => typed_character(keystroke).map(TerminalKey::Character),
    }
}

fn typed_character(keystroke: &Keystroke) -> Option<char> {
    keystroke
        .key_char
        .as_deref()
        .and_then(single_character)
        .or_else(|| single_character(&keystroke.key))
}

fn single_character(text: &str) -> Option<char> {
    let mut chars = text.chars();
    let ch = chars.next()?;
    chars.next().is_none().then_some(ch)
}

fn resize_terminal_entry(
    surface_id: SurfaceId,
    bounds: Bounds<Pixels>,
    surface_state: &TerminalSurfaceState,
    handle: Option<&TerminalEntryHandle>,
) {
    let metrics = TerminalMetrics::new(8.0, 16.0);
    let size = metrics.cols_rows(bounds.size.width.as_f32(), bounds.size.height.as_f32());
    if let Some((cols, rows)) = surface_state.grid_size_changed(surface_id, size.cols, size.rows)
        && let Some(handle) = handle
    {
        handle.resize(cols, rows);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Keystroke;
    use umux_terminal::TerminalInputRoute;

    #[test]
    fn plain_character_routes_to_write_bytes() {
        let keystroke = Keystroke::parse("a").unwrap().with_simulated_ime();

        assert_eq!(
            route_gpui_keystroke(&keystroke, false),
            TerminalInputRoute::WriteBytes(b"a".to_vec())
        );
    }

    #[test]
    fn shifted_character_routes_typed_key_char() {
        let keystroke = Keystroke::parse("A").unwrap().with_simulated_ime();

        assert_eq!(
            route_gpui_keystroke(&keystroke, false),
            TerminalInputRoute::WriteBytes(b"A".to_vec())
        );
    }

    #[test]
    fn ime_character_prefers_key_char_over_key_label() {
        let keystroke = Keystroke {
            modifiers: Default::default(),
            key: "q".to_string(),
            key_char: Some("ß".to_string()),
        };

        assert_eq!(
            route_gpui_keystroke(&keystroke, false),
            TerminalInputRoute::WriteBytes("ß".as_bytes().to_vec())
        );
    }

    #[test]
    fn enter_keystroke_keeps_terminal_carriage_return_route() {
        let keystroke = Keystroke::parse("enter").unwrap().with_simulated_ime();

        assert_eq!(
            route_gpui_keystroke(&keystroke, false),
            TerminalInputRoute::WriteBytes(b"\r".to_vec())
        );
    }

    #[test]
    fn selected_text_for_clipboard_ignores_missing_or_empty_selection() {
        assert_eq!(selected_text_for_clipboard(None), None);
        assert_eq!(
            selected_text_for_clipboard(Some(&snapshot("abcd", None))),
            None
        );
    }

    #[test]
    fn selected_text_for_clipboard_returns_snapshot_selection() {
        let snapshot = snapshot(
            "abcd",
            Some(umux_terminal::TerminalSelection {
                start_col: 1,
                start_row: 0,
                end_col: 2,
                end_row: 0,
            }),
        );

        assert_eq!(
            selected_text_for_clipboard(Some(&snapshot)),
            Some("bc".to_string())
        );
    }

    #[test]
    fn ctrl_shift_c_routes_to_copy_when_selection_exists() {
        let keystroke = Keystroke::parse("ctrl-shift-c").unwrap();

        assert_eq!(
            route_gpui_keystroke(&keystroke, true),
            TerminalInputRoute::CopySelection
        );
    }

    #[test]
    fn resize_state_reports_only_changed_grid_sizes() {
        let state = TerminalSurfaceState::new();
        let surface_id = SurfaceId(42);

        assert_eq!(state.grid_size_changed(surface_id, 10, 5), Some((10, 5)));
        assert_eq!(state.grid_size_changed(surface_id, 10, 5), None);
        assert_eq!(state.grid_size_changed(surface_id, 12, 5), Some((12, 5)));
    }

    fn snapshot(
        text: &str,
        selection: Option<umux_terminal::TerminalSelection>,
    ) -> TerminalRendererSnapshot {
        TerminalRendererSnapshot {
            cols: text.len() as u16,
            rows: 1,
            cells: text.chars().map(cell).collect(),
            cursor: umux_terminal::TerminalCursor {
                col: 0,
                row: 0,
                visible: false,
            },
            selection,
            scrollback_lines: 0,
            version: 1,
        }
    }

    fn cell(ch: char) -> umux_terminal::TerminalCell {
        umux_terminal::TerminalCell {
            ch,
            fg: umux_terminal::TerminalColor::rgb(255, 255, 255),
            bg: umux_terminal::TerminalColor::rgb(0, 0, 0),
            bold: false,
            italic: false,
            underline: false,
            inverse: false,
        }
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShortcutBinding {
    pub cmux_shortcut: String,
    pub windows_shortcut: Option<String>,
    pub windows_bindings: Vec<WindowsBinding>,
    pub action: String,
    pub configurable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct WindowsBinding {
    pub chord: String,
    pub context: ShortcutContext,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutContext {
    Global,
    TerminalFocused,
    SelectionPresent,
    NonTerminal,
}

pub fn default_shortcuts() -> Vec<ShortcutBinding> {
    vec![
        global_binding("Cmd+N", "Ctrl+N", "new_workspace"),
        global_binding("Cmd+1-8", "Ctrl+1-8", "jump_workspace_1_8"),
        global_binding("Cmd+9", "Ctrl+9", "jump_last_workspace"),
        global_binding("Cmd+Shift+W", "Ctrl+Shift+W", "close_workspace"),
        global_binding("Cmd+B", "Ctrl+B", "toggle_sidebar"),
        global_binding("Cmd+T", "Ctrl+T", "new_surface"),
        global_binding("Cmd+W", "Ctrl+W", "close_surface"),
        global_binding("Cmd+D", "Ctrl+Alt+D", "split_right"),
        global_binding("Cmd+Shift+D", "Ctrl+Shift+Alt+D", "split_down"),
        global_binding("Cmd+Shift+L", "Ctrl+Shift+L", "open_browser_split"),
        global_binding("Cmd+L", "Ctrl+L", "focus_address_bar"),
        global_binding("Cmd+I", "Ctrl+I", "show_notifications"),
        global_binding("Cmd+Shift+U", "Ctrl+Shift+U", "jump_latest_unread"),
        global_binding("Cmd+K", "Ctrl+Shift+K", "clear_scrollback"),
        binding(
            "Cmd+C",
            Some("Ctrl+Shift+C when terminal focused, Ctrl+C with selection"),
            vec![
                windows_binding("Ctrl+Shift+C", ShortcutContext::TerminalFocused),
                windows_binding("Ctrl+C", ShortcutContext::SelectionPresent),
            ],
            "copy",
        ),
        binding(
            "Cmd+V",
            Some("Ctrl+Shift+V when terminal focused, Ctrl+V elsewhere"),
            vec![
                windows_binding("Ctrl+Shift+V", ShortcutContext::TerminalFocused),
                windows_binding("Ctrl+V", ShortcutContext::NonTerminal),
            ],
            "paste",
        ),
        global_binding("Cmd+,", "Ctrl+,", "settings"),
        binding("Cmd+Q", None, Vec::new(), "quit"),
    ]
}

fn global_binding(
    cmux_shortcut: &'static str,
    windows_shortcut: &'static str,
    action: &'static str,
) -> ShortcutBinding {
    binding(
        cmux_shortcut,
        Some(windows_shortcut),
        vec![windows_binding(windows_shortcut, ShortcutContext::Global)],
        action,
    )
}

fn binding(
    cmux_shortcut: &'static str,
    windows_shortcut: Option<&'static str>,
    windows_bindings: Vec<WindowsBinding>,
    action: &'static str,
) -> ShortcutBinding {
    ShortcutBinding {
        cmux_shortcut: cmux_shortcut.to_string(),
        windows_shortcut: windows_shortcut.map(str::to_string),
        windows_bindings,
        action: action.to_string(),
        configurable: true,
    }
}

fn windows_binding(chord: &'static str, context: ShortcutContext) -> WindowsBinding {
    WindowsBinding {
        chord: chord.to_string(),
        context,
    }
}

#[cfg(test)]
mod tests {
    use super::{ShortcutContext, WindowsBinding, default_shortcuts};
    use std::collections::HashSet;

    #[test]
    fn quit_is_unbound_by_default() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "quit")
            .expect("quit binding should exist");

        assert_eq!(binding.windows_shortcut.as_deref(), None);
    }

    #[test]
    fn split_right_avoids_ctrl_d() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "split_right")
            .expect("split_right binding should exist");

        assert_eq!(binding.windows_shortcut.as_deref(), Some("Ctrl+Alt+D"));
    }

    #[test]
    fn terminal_copy_uses_context_sensitive_windows_terminal_binding() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "copy")
            .expect("copy binding should exist");

        assert_eq!(
            binding.windows_shortcut.as_deref(),
            Some("Ctrl+Shift+C when terminal focused, Ctrl+C with selection")
        );
        assert!(binding.configurable);
    }

    #[test]
    fn default_shortcuts_have_no_duplicate_actions() {
        let mut actions = HashSet::new();

        for binding in default_shortcuts() {
            assert!(actions.insert(binding.action), "duplicate action");
        }
    }

    #[test]
    fn default_shortcuts_have_no_duplicate_windows_binding_pairs() {
        let mut bindings = HashSet::new();

        for shortcut in default_shortcuts() {
            for binding in shortcut.windows_bindings {
                assert!(
                    bindings.insert((binding.context, binding.chord)),
                    "duplicate windows binding pair"
                );
            }
        }
    }

    #[test]
    fn copy_has_machine_readable_windows_bindings() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "copy")
            .expect("copy binding should exist");

        assert_eq!(
            binding.windows_bindings,
            vec![
                WindowsBinding {
                    chord: "Ctrl+Shift+C".to_string(),
                    context: ShortcutContext::TerminalFocused,
                },
                WindowsBinding {
                    chord: "Ctrl+C".to_string(),
                    context: ShortcutContext::SelectionPresent,
                },
            ]
        );
    }
}

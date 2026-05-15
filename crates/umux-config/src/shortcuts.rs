// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ShortcutBinding {
    pub cmux_shortcut: &'static str,
    pub windows_shortcut: Option<&'static str>,
    pub action: &'static str,
    pub configurable: bool,
}

pub fn default_shortcuts() -> Vec<ShortcutBinding> {
    vec![
        binding("Cmd+N", Some("Ctrl+N"), "new_workspace"),
        binding("Cmd+1-8", Some("Ctrl+1-8"), "jump_workspace_1_8"),
        binding("Cmd+9", Some("Ctrl+9"), "jump_last_workspace"),
        binding("Cmd+Shift+W", Some("Ctrl+Shift+W"), "close_workspace"),
        binding("Cmd+B", Some("Ctrl+B"), "toggle_sidebar"),
        binding("Cmd+T", Some("Ctrl+T"), "new_surface"),
        binding("Cmd+W", Some("Ctrl+W"), "close_surface"),
        binding("Cmd+D", Some("Ctrl+Alt+D"), "split_right"),
        binding("Cmd+Shift+D", Some("Ctrl+Shift+Alt+D"), "split_down"),
        binding("Cmd+Shift+L", Some("Ctrl+Shift+L"), "open_browser_split"),
        binding("Cmd+L", Some("Ctrl+L"), "focus_address_bar"),
        binding("Cmd+I", Some("Ctrl+I"), "show_notifications"),
        binding("Cmd+Shift+U", Some("Ctrl+Shift+U"), "jump_latest_unread"),
        binding("Cmd+K", Some("Ctrl+Shift+K"), "clear_scrollback"),
        binding(
            "Cmd+C",
            Some("Ctrl+Shift+C when terminal focused, Ctrl+C with selection"),
            "copy",
        ),
        binding(
            "Cmd+V",
            Some("Ctrl+Shift+V when terminal focused, Ctrl+V elsewhere"),
            "paste",
        ),
        binding("Cmd+,", Some("Ctrl+,"), "settings"),
        binding("Cmd+Q", None, "quit"),
    ]
}

fn binding(
    cmux_shortcut: &'static str,
    windows_shortcut: Option<&'static str>,
    action: &'static str,
) -> ShortcutBinding {
    ShortcutBinding {
        cmux_shortcut,
        windows_shortcut,
        action,
        configurable: true,
    }
}

#[cfg(test)]
mod tests {
    use super::default_shortcuts;

    #[test]
    fn quit_is_unbound_by_default() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "quit")
            .expect("quit binding should exist");

        assert_eq!(binding.windows_shortcut, None);
    }

    #[test]
    fn split_right_avoids_ctrl_d() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "split_right")
            .expect("split_right binding should exist");

        assert_eq!(binding.windows_shortcut, Some("Ctrl+Alt+D"));
    }

    #[test]
    fn terminal_copy_uses_context_sensitive_windows_terminal_binding() {
        let binding = default_shortcuts()
            .into_iter()
            .find(|binding| binding.action == "copy")
            .expect("copy binding should exist");

        assert_eq!(
            binding.windows_shortcut,
            Some("Ctrl+Shift+C when terminal focused, Ctrl+C with selection")
        );
        assert!(binding.configurable);
    }
}

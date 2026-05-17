// SPDX-License-Identifier: GPL-3.0-or-later

pub fn format_keybinding(chord: &str) -> String {
    chord.replace('+', " + ")
}

#[cfg(test)]
mod tests {
    use super::format_keybinding;

    #[test]
    fn keybinding_adds_spacing_between_keys() {
        assert_eq!(format_keybinding("Ctrl+Shift+U"), "Ctrl + Shift + U");
    }
}

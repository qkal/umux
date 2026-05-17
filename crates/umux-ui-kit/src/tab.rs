// SPDX-License-Identifier: GPL-3.0-or-later

pub fn tab_unread_marker(unread: bool) -> &'static str {
    if unread { " *" } else { "" }
}

pub fn tab_label(title: &str, unread: bool) -> String {
    format!("{}{}", title, tab_unread_marker(unread))
}

#[cfg(test)]
mod tests {
    use super::tab_label;

    #[test]
    fn tab_label_marks_unread_tabs() {
        assert_eq!(tab_label("cargo test", true), "cargo test *");
        assert_eq!(tab_label("shell", false), "shell");
    }
}

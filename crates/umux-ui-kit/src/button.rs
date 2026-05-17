// SPDX-License-Identifier: GPL-3.0-or-later

pub fn button_label(label: &str) -> String {
    label.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::button_label;

    #[test]
    fn button_label_trims_outer_whitespace() {
        assert_eq!(button_label(" jump "), "jump");
    }
}

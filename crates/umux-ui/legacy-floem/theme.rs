// SPDX-License-Identifier: GPL-3.0-or-later

pub const SIDEBAR_WIDTH: f64 = 260.0;
pub const TOP_BAR_HEIGHT: f64 = 38.0;
pub const SURFACE_TAB_HEIGHT: f64 = 30.0;
pub const UNREAD_BLUE_HEX: &str = "#2f80ff";
pub const BACKGROUND_HEX: &str = "#111316";
pub const PANEL_HEX: &str = "#181b20";
pub const TEXT_HEX: &str = "#e7eaf0";
pub const MUTED_TEXT_HEX: &str = "#9ba3af";

#[cfg(test)]
mod tests {
    use super::{SIDEBAR_WIDTH, UNREAD_BLUE_HEX};

    #[test]
    fn sidebar_width_matches_compact_cmux_density() {
        assert_eq!(SIDEBAR_WIDTH, 260.0);
    }

    #[test]
    fn unread_blue_is_the_attention_color() {
        assert_eq!(UNREAD_BLUE_HEX, "#2f80ff");
    }
}

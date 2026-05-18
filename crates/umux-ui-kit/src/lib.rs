// SPDX-License-Identifier: GPL-3.0-or-later

pub mod button;
pub mod icon_button;
pub mod keybinding;
pub mod tab;
pub mod theme;

pub use button::button_label;
pub use icon_button::IconName;
pub use keybinding::format_keybinding;
pub use tab::{tab_label, tab_unread_marker};
pub use theme::{
    ACCENT, ACTIVE, BACKGROUND, BORDER, BORDER_STRONG, DIM_TEXT, ELEVATED, HOVER, MUTED_TEXT,
    PANEL, SURFACE, TEXT, UNREAD_BLUE, WARNING, WARNING_TEXT,
};

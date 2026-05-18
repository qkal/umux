// SPDX-License-Identifier: GPL-3.0-or-later

use gpui::Hsla;

pub const BACKGROUND: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.12,
    l: 0.075,
    a: 1.0,
};
pub const SURFACE: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.12,
    l: 0.10,
    a: 1.0,
};
pub const PANEL: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.11,
    l: 0.14,
    a: 1.0,
};
pub const ELEVATED: Hsla = Hsla {
    h: 222.0 / 360.0,
    s: 0.11,
    l: 0.18,
    a: 1.0,
};
pub const HOVER: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.10,
    l: 0.20,
    a: 1.0,
};
pub const ACTIVE: Hsla = Hsla {
    h: 222.0 / 360.0,
    s: 0.11,
    l: 0.24,
    a: 1.0,
};
pub const BORDER: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.10,
    l: 0.23,
    a: 1.0,
};
pub const BORDER_STRONG: Hsla = Hsla {
    h: 215.0 / 360.0,
    s: 0.28,
    l: 0.44,
    a: 1.0,
};
pub const TEXT: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.18,
    l: 0.91,
    a: 1.0,
};
pub const MUTED_TEXT: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.11,
    l: 0.66,
    a: 1.0,
};
pub const DIM_TEXT: Hsla = Hsla {
    h: 220.0 / 360.0,
    s: 0.08,
    l: 0.48,
    a: 1.0,
};
pub const ACCENT: Hsla = Hsla {
    h: 212.0 / 360.0,
    s: 0.90,
    l: 0.62,
    a: 1.0,
};
pub const WARNING: Hsla = Hsla {
    h: 42.0 / 360.0,
    s: 0.45,
    l: 0.15,
    a: 1.0,
};
pub const WARNING_TEXT: Hsla = Hsla {
    h: 42.0 / 360.0,
    s: 0.78,
    l: 0.74,
    a: 1.0,
};
pub const UNREAD_BLUE: Hsla = ACCENT;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_shell_tokens_are_ordered_from_dark_to_active() {
        assert!(BACKGROUND.l < SURFACE.l);
        assert!(SURFACE.l < PANEL.l);
        assert!(PANEL.l < ELEVATED.l);
        assert!(ELEVATED.l < ACTIVE.l);
    }

    #[test]
    fn warning_text_is_brighter_than_warning_background() {
        assert!(WARNING_TEXT.l > WARNING.l);
    }
}

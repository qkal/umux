// SPDX-License-Identifier: GPL-3.0-or-later

use umux_core::model::SurfaceKind;

pub fn unsupported_surface_message(kind: SurfaceKind, title: &str) -> String {
    let surface = match kind {
        SurfaceKind::Terminal => "terminal",
        SurfaceKind::Browser => "browser",
    };

    format!("Unsupported {surface} surface: {title}")
}

#[cfg(test)]
mod tests {
    use umux_core::model::SurfaceKind;

    use super::unsupported_surface_message;

    #[test]
    fn unsupported_surface_message_preserves_browser_title() {
        assert_eq!(
            unsupported_surface_message(SurfaceKind::Browser, "https://example.com"),
            "Unsupported browser surface: https://example.com"
        );
    }
}

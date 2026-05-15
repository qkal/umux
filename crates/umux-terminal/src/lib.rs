// SPDX-License-Identifier: GPL-3.0-or-later

pub mod engine;

pub use engine::TerminalSurface;

pub const CRATE_NAME: &str = "umux-terminal";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn terminal_feed_extracts_osc_notifications() {
        let mut surface = TerminalSurface::new("C:/work/alpha");
        let events = surface.feed_output("ok\u{1b}]9;ready\u{7}");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "ready");
        assert!(surface.scrollback().contains("ok"));
    }
}

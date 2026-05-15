// SPDX-License-Identifier: GPL-3.0-or-later

pub mod engine;
pub mod shell;
pub mod startup_env;

pub use engine::TerminalSurface;
pub use shell::{ResolvedShell, ShellResolver};
pub use startup_env::StartupEnvironment;

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

    #[test]
    fn terminal_feed_extracts_osc_notifications_split_across_chunks() {
        let mut surface = TerminalSurface::new("C:/work/alpha");

        assert!(surface.feed_output("ok\u{1b}]9;rea").is_empty());
        let events = surface.feed_output("dy\u{7}");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "ready");
        assert!(surface.scrollback().contains("ok\u{1b}]9;ready\u{7}"));
    }

    #[test]
    fn terminal_feed_does_not_keep_plain_output_pending() {
        let mut surface = TerminalSurface::new("C:/work/alpha");

        surface.feed_output("ordinary output\nwith no osc\n");

        assert_eq!(surface.pending_len(), 0);
    }
}

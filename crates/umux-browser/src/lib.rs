// SPDX-License-Identifier: GPL-3.0-or-later

pub mod automation;

pub use automation::{BrowserCommand, BrowserSurfaceState};

pub const CRATE_NAME: &str = "umux-browser";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_command_records_open_url() {
        let mut state = BrowserSurfaceState::new();
        state.apply(BrowserCommand::OpenUrl("https://example.com".to_string()));

        assert_eq!(state.current_url.as_deref(), Some("https://example.com"));
        assert_eq!(state.history, vec!["https://example.com"]);
    }
}

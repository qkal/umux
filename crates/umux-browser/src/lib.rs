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

    #[test]
    fn browser_command_moves_back_and_forward_through_history() {
        let mut state = BrowserSurfaceState::new();
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/one".to_string(),
        ));
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/two".to_string(),
        ));

        state.apply(BrowserCommand::Back);

        assert_eq!(
            state.current_url.as_deref(),
            Some("https://example.com/one")
        );
        assert!(!state.can_go_back);
        assert!(state.can_go_forward);

        state.apply(BrowserCommand::Forward);

        assert_eq!(
            state.current_url.as_deref(),
            Some("https://example.com/two")
        );
        assert!(state.can_go_back);
        assert!(!state.can_go_forward);
    }

    #[test]
    fn browser_open_url_after_back_clears_forward_history() {
        let mut state = BrowserSurfaceState::new();
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/one".to_string(),
        ));
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/two".to_string(),
        ));
        state.apply(BrowserCommand::Back);

        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/three".to_string(),
        ));

        assert_eq!(
            state.history,
            vec!["https://example.com/one", "https://example.com/three"]
        );
        assert_eq!(
            state.current_url.as_deref(),
            Some("https://example.com/three")
        );
        assert!(state.can_go_back);
        assert!(!state.can_go_forward);
    }

    #[test]
    fn browser_open_url_from_middle_replaces_all_forward_history() {
        let mut state = BrowserSurfaceState::new();
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/one".to_string(),
        ));
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/two".to_string(),
        ));
        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/three".to_string(),
        ));
        state.apply(BrowserCommand::Back);
        state.apply(BrowserCommand::Back);

        state.apply(BrowserCommand::OpenUrl(
            "https://example.com/four".to_string(),
        ));
        state.apply(BrowserCommand::Forward);

        assert_eq!(
            state.history,
            vec!["https://example.com/one", "https://example.com/four"]
        );
        assert_eq!(
            state.current_url.as_deref(),
            Some("https://example.com/four")
        );
        assert!(state.can_go_back);
        assert!(!state.can_go_forward);
    }
}

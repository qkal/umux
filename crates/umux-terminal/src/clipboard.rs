// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use thiserror::Error;

#[derive(Debug, Error, Eq, PartialEq)]
pub enum ClipboardError {
    #[error("clipboard unavailable: {0}")]
    Unavailable(String),
}

pub trait ClipboardPort {
    fn store_text(&self, text: &str) -> Result<(), ClipboardError>;
    fn load_text(&self) -> Result<String, ClipboardError>;
}

#[derive(Clone, Debug, Default)]
pub struct FakeClipboard {
    text: Arc<Mutex<String>>,
}

impl ClipboardPort for FakeClipboard {
    fn store_text(&self, text: &str) -> Result<(), ClipboardError> {
        *self
            .text
            .lock()
            .map_err(|_| ClipboardError::Unavailable("lock poisoned".to_string()))? =
            text.to_string();
        Ok(())
    }

    fn load_text(&self) -> Result<String, ClipboardError> {
        self.text
            .lock()
            .map(|text| text.clone())
            .map_err(|_| ClipboardError::Unavailable("lock poisoned".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_clipboard_stores_and_loads_text() {
        let clipboard = FakeClipboard::default();

        clipboard.store_text("hello").unwrap();

        assert_eq!(clipboard.load_text().unwrap(), "hello");
    }

    #[test]
    fn fake_clipboard_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<FakeClipboard>();
    }
}

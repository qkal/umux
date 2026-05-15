// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use umux_notify::{TerminalNotification, parse_osc_notifications};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalSurface {
    cwd: String,
    scrollback: String,
}

impl TerminalSurface {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            scrollback: String::new(),
        }
    }

    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    pub fn scrollback(&self) -> &str {
        &self.scrollback
    }

    pub fn feed_output(&mut self, output: &str) -> Vec<TerminalNotification> {
        self.scrollback.push_str(output);
        parse_osc_notifications(output)
    }
}

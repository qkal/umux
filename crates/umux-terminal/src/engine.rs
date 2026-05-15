// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use umux_notify::{TerminalNotification, parse_osc_notifications};

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalSurface {
    cwd: String,
    scrollback: String,
    #[serde(default)]
    pending_output: String,
}

impl TerminalSurface {
    pub fn new(cwd: impl Into<String>) -> Self {
        Self {
            cwd: cwd.into(),
            scrollback: String::new(),
            pending_output: String::new(),
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

        let buffered_output = if self.pending_output.is_empty() {
            output.to_owned()
        } else {
            let mut buffered_output =
                String::with_capacity(self.pending_output.len() + output.len());
            buffered_output.push_str(&self.pending_output);
            buffered_output.push_str(output);
            buffered_output
        };

        self.pending_output = trailing_incomplete_osc(&buffered_output)
            .map(str::to_owned)
            .unwrap_or_default();

        parse_osc_notifications(&buffered_output)
    }

    #[cfg(test)]
    pub(crate) fn pending_len(&self) -> usize {
        self.pending_output.len()
    }
}

fn trailing_incomplete_osc(output: &str) -> Option<&str> {
    let osc_start = output.rfind("\u{1b}]")?;
    let trailing = &output[osc_start..];

    if trailing.contains('\u{7}') || trailing.contains("\u{1b}\\") {
        None
    } else {
        Some(trailing)
    }
}

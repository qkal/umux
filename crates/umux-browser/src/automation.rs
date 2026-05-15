// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum BrowserCommand {
    OpenUrl(String),
    Back,
    Forward,
    Reload,
    EvaluateJavaScript(String),
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct BrowserSurfaceState {
    pub current_url: Option<String>,
    pub history: Vec<String>,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    #[serde(default)]
    history_index: Option<usize>,
}

impl BrowserSurfaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, command: BrowserCommand) {
        match command {
            BrowserCommand::OpenUrl(url) => {
                if let Some(index) = self.history_index {
                    self.history.truncate(index + 1);
                }

                self.current_url = Some(url.clone());
                self.history.push(url);
                self.history_index = self.history.len().checked_sub(1);
                self.update_navigation_flags();
            }
            BrowserCommand::Back => {
                if let Some(index) = self.history_index.filter(|index| *index > 0) {
                    let previous_index = index - 1;
                    self.history_index = Some(previous_index);
                    self.current_url = self.history.get(previous_index).cloned();
                }
                self.update_navigation_flags();
            }
            BrowserCommand::Forward => {
                if let Some(index) = self
                    .history_index
                    .filter(|index| *index + 1 < self.history.len())
                {
                    let next_index = index + 1;
                    self.history_index = Some(next_index);
                    self.current_url = self.history.get(next_index).cloned();
                }
                self.update_navigation_flags();
            }
            BrowserCommand::Reload | BrowserCommand::EvaluateJavaScript(_) => {}
        }
    }

    fn update_navigation_flags(&mut self) {
        self.can_go_back = self.history_index.is_some_and(|index| index > 0);
        self.can_go_forward = self
            .history_index
            .is_some_and(|index| index + 1 < self.history.len());
    }
}

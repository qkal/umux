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
}

impl BrowserSurfaceState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, command: BrowserCommand) {
        match command {
            BrowserCommand::OpenUrl(url) => {
                self.current_url = Some(url.clone());
                self.history.push(url);
                self.can_go_back = self.history.len() > 1;
                self.can_go_forward = false;
            }
            BrowserCommand::Back => {
                self.can_go_forward = self.current_url.is_some();
            }
            BrowserCommand::Forward => {
                self.can_go_forward = false;
            }
            BrowserCommand::Reload | BrowserCommand::EvaluateJavaScript(_) => {}
        }
    }
}

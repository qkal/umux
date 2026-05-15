// SPDX-License-Identifier: GPL-3.0-or-later

use clap::{Parser, Subcommand};
use serde_json::json;
use thiserror::Error;
use umux_ipc::{Method, RequestFrame};

#[derive(Debug, Parser)]
#[command(name = "umux-cli")]
pub struct Cli {
    #[arg(long)]
    pub dry_run_json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Ping,
    NewWorkspace {
        cwd: String,
    },
    Split {
        axis: String,
    },
    Notify {
        message: String,
    },
    Browser {
        #[command(subcommand)]
        command: BrowserCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum BrowserCommand {
    Open { url: String },
}

#[derive(Debug, Error)]
pub enum CliError {
    #[error("IPC transport is required for this command")]
    RequiresServer,
}

impl Cli {
    pub fn into_request(self, id: u64) -> Result<RequestFrame, CliError> {
        let (method, params) = match self.command {
            Command::Ping => (Method::SystemPing, json!({})),
            Command::NewWorkspace { cwd } => (Method::WorkspaceCreate, json!({ "cwd": cwd })),
            Command::Split { axis } => (Method::PaneSplit, json!({ "axis": axis })),
            Command::Notify { message } => {
                (Method::NotificationCreate, json!({ "message": message }))
            }
            Command::Browser {
                command: BrowserCommand::Open { url },
            } => (Method::BrowserOpen, json!({ "url": url })),
        };

        Ok(RequestFrame::new(id, method, params))
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser as _;
    use umux_ipc::Method;

    use super::Cli;

    #[test]
    fn builds_workspace_create_request() {
        let cli = Cli::parse_from([
            "umux-cli",
            "--dry-run-json",
            "new-workspace",
            "C:/work/alpha",
        ]);

        let request = cli.into_request(1).expect("request should build");

        assert_eq!(request.id, 1);
        assert_eq!(request.method, Method::WorkspaceCreate);
        assert_eq!(request.params["cwd"], "C:/work/alpha");
    }

    #[test]
    fn builds_browser_open_request() {
        let cli = Cli::parse_from([
            "umux-cli",
            "--dry-run-json",
            "browser",
            "open",
            "https://example.com",
        ]);

        let request = cli.into_request(9).expect("request should build");

        assert_eq!(request.id, 9);
        assert_eq!(request.method, Method::BrowserOpen);
        assert_eq!(request.params["url"], "https://example.com");
    }

    #[test]
    fn builds_notification_request() {
        let cli = Cli::parse_from(["umux-cli", "--dry-run-json", "notify", "Needs input"]);

        let request = cli.into_request(3).expect("request should build");

        assert_eq!(request.id, 3);
        assert_eq!(request.method, Method::NotificationCreate);
        assert_eq!(request.params["message"], "Needs input");
    }
}

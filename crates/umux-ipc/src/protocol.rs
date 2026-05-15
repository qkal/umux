// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Method {
    #[serde(rename = "system.ping")]
    SystemPing,
    #[serde(rename = "system.capabilities")]
    SystemCapabilities,
    #[serde(rename = "workspace.create")]
    WorkspaceCreate,
    #[serde(rename = "pane.split")]
    PaneSplit,
    #[serde(rename = "browser.open")]
    BrowserOpen,
    #[serde(rename = "notification.create")]
    NotificationCreate,
    #[serde(rename = "events.stream")]
    EventsStream,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestFrame {
    pub id: u64,
    pub method: Method,
    pub params: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResponseFrame {
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<ErrorFrame>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorFrame {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl RequestFrame {
    #[must_use]
    pub fn new(id: u64, method: Method, params: Value) -> Self {
        Self { id, method, params }
    }

    pub fn to_json_line(&self) -> Result<String, ProtocolError> {
        let mut line = serde_json::to_string(self)?;
        line.push('\n');
        Ok(line)
    }

    pub fn from_json_line(line: &str) -> Result<Self, ProtocolError> {
        Ok(serde_json::from_str(line)?)
    }
}

impl ResponseFrame {
    #[must_use]
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
        }
    }

    #[must_use]
    pub fn error(id: u64, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            result: None,
            error: Some(ErrorFrame {
                code: code.into(),
                message: message.into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Method, RequestFrame, ResponseFrame};

    #[test]
    fn encodes_workspace_create_request_as_single_json_line() {
        let frame = RequestFrame::new(1, Method::WorkspaceCreate, json!({"cwd":"C:/work/alpha"}));

        let line = frame.to_json_line().expect("request should encode");

        assert!(line.ends_with('\n'));
        assert!(line.contains("\"method\":\"workspace.create\""));
        assert!(line.contains("\"cwd\":\"C:/work/alpha\""));
    }

    #[test]
    fn decodes_events_stream_method() {
        let frame = RequestFrame::from_json_line(
            "{\"id\":7,\"method\":\"events.stream\",\"params\":{\"since\":0}}\n",
        )
        .expect("request should decode");

        assert_eq!(frame.id, 7);
        assert_eq!(frame.method, Method::EventsStream);
    }

    #[test]
    fn response_error_is_structured() {
        let frame = ResponseFrame::error(2, "unknown_method", "method is not supported");

        let error = frame.error.expect("error frame should be present");
        assert_eq!(error.code, "unknown_method");
    }
}

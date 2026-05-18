// SPDX-License-Identifier: GPL-3.0-or-later

use serde::de::Error as _;
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
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

#[derive(Debug, Clone, PartialEq)]
pub struct ResponseFrame {
    pub id: u64,
    payload: ResponsePayload,
}

#[derive(Debug, Clone, PartialEq)]
enum ResponsePayload {
    Result(Value),
    Error(ErrorFrame),
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
            payload: ResponsePayload::Result(result),
        }
    }

    #[must_use]
    pub fn error(id: u64, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            id,
            payload: ResponsePayload::Error(ErrorFrame {
                code: code.into(),
                message: message.into(),
            }),
        }
    }

    #[must_use]
    pub fn result(&self) -> Option<&Value> {
        match &self.payload {
            ResponsePayload::Result(result) => Some(result),
            ResponsePayload::Error(_) => None,
        }
    }

    #[must_use]
    pub fn error_frame(&self) -> Option<&ErrorFrame> {
        match &self.payload {
            ResponsePayload::Result(_) => None,
            ResponsePayload::Error(error) => Some(error),
        }
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

impl Serialize for ResponseFrame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("id", &self.id)?;
        match &self.payload {
            ResponsePayload::Result(result) => map.serialize_entry("result", result)?,
            ResponsePayload::Error(error) => map.serialize_entry("error", error)?,
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for ResponseFrame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut object = Map::deserialize(deserializer)?;
        let id_value = object
            .remove("id")
            .ok_or_else(|| D::Error::missing_field("id"))?;
        let id = serde_json::from_value(id_value).map_err(D::Error::custom)?;

        let result = object.remove("result");
        let error = object.remove("error");

        let payload = match (result, error) {
            (Some(result), None) => ResponsePayload::Result(result),
            (None, Some(error)) => {
                let error = serde_json::from_value(error).map_err(D::Error::custom)?;
                ResponsePayload::Error(error)
            }
            (Some(_), Some(_)) => {
                return Err(D::Error::custom(
                    "response frame must contain either result or error, not both",
                ));
            }
            (None, None) => {
                return Err(D::Error::custom(
                    "response frame must contain result or error",
                ));
            }
        };

        Ok(Self { id, payload })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

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

        let error = frame.error_frame().expect("error frame should be present");
        assert_eq!(error.code, "unknown_method");
    }

    #[test]
    fn ok_response_serializes_without_error_key() {
        let frame = ResponseFrame::ok(3, json!({"pong":true}));

        let line = frame.to_json_line().expect("response should encode");

        assert!(line.contains("\"result\":{\"pong\":true}"));
        assert!(!line.contains("\"error\""));
    }

    #[test]
    fn error_response_serializes_without_result_key() {
        let frame = ResponseFrame::error(4, "unknown_method", "method is not supported");

        let line = frame.to_json_line().expect("response should encode");

        assert!(line.contains("\"error\":{\"code\":\"unknown_method\""));
        assert!(!line.contains("\"result\""));
    }

    #[test]
    fn error_response_round_trips_with_structured_error() {
        let frame = ResponseFrame::error(10, "invalid_params", "axis is required");

        let line = frame.to_json_line().expect("response should encode");
        let decoded = ResponseFrame::from_json_line(&line).expect("response should decode");

        assert_eq!(decoded.id, 10);
        assert_eq!(decoded.result(), None);
        let error = decoded
            .error_frame()
            .expect("error frame should be present");
        assert_eq!(error.code, "invalid_params");
        assert_eq!(error.message, "axis is required");
    }

    #[test]
    fn null_result_response_round_trips_as_result_present() {
        let frame = ResponseFrame::ok(5, Value::Null);

        let line = frame.to_json_line().expect("response should encode");
        let decoded = ResponseFrame::from_json_line(&line).expect("response should decode");

        assert_eq!(decoded.result(), Some(&Value::Null));
        assert_eq!(decoded.error_frame(), None);
    }

    #[test]
    fn malformed_response_with_both_result_and_error_is_rejected() {
        let decoded = ResponseFrame::from_json_line(
            "{\"id\":6,\"result\":null,\"error\":{\"code\":\"boom\",\"message\":\"bad\"}}\n",
        );

        assert!(decoded.is_err());
    }

    #[test]
    fn malformed_response_with_neither_result_nor_error_is_rejected() {
        let decoded = ResponseFrame::from_json_line("{\"id\":6}\n");

        assert!(decoded.is_err());
    }

    #[test]
    fn request_crlf_decode_works() {
        let frame = RequestFrame::from_json_line(
            "{\"id\":7,\"method\":\"events.stream\",\"params\":{\"since\":0}}\r\n",
        )
        .expect("request should decode");

        assert_eq!(frame.id, 7);
        assert_eq!(frame.method, Method::EventsStream);
    }

    #[test]
    fn invalid_request_json_returns_protocol_error() {
        let decoded = RequestFrame::from_json_line("{\"id\":");

        assert!(matches!(decoded, Err(super::ProtocolError::Json(_))));
    }

    #[test]
    fn unknown_request_method_returns_protocol_error() {
        let decoded = RequestFrame::from_json_line(
            "{\"id\":8,\"method\":\"workspace.destroy\",\"params\":{}}\n",
        );

        assert!(matches!(decoded, Err(super::ProtocolError::Json(_))));
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OscNotificationKind {
    #[serde(rename = "osc9")]
    Osc9,
    #[serde(rename = "osc99")]
    Osc99,
    #[serde(rename = "osc777")]
    Osc777,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalNotification {
    pub kind: OscNotificationKind,
    pub title: Option<String>,
    pub message: String,
    #[serde(with = "time::serde::rfc3339")]
    pub received_at: OffsetDateTime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_notification_with_stable_kind_and_rfc3339_timestamp() {
        let notification = TerminalNotification {
            kind: OscNotificationKind::Osc9,
            title: None,
            message: "ready".to_owned(),
            received_at: OffsetDateTime::UNIX_EPOCH,
        };

        let json = serde_json::to_value(notification).expect("notification serializes");

        assert_eq!(json["kind"], "osc9");
        assert!(json["received_at"].is_string());
        assert_eq!(json["received_at"], "1970-01-01T00:00:00Z");
    }
}

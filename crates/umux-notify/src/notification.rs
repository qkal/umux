// SPDX-License-Identifier: GPL-3.0-or-later

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum OscNotificationKind {
    Osc9,
    Osc99,
    Osc777,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct TerminalNotification {
    pub kind: OscNotificationKind,
    pub title: Option<String>,
    pub message: String,
    pub received_at: OffsetDateTime,
}

// SPDX-License-Identifier: GPL-3.0-or-later

pub mod notification;
pub mod osc;

pub use notification::{OscNotificationKind, TerminalNotification};
pub use osc::parse_osc_notifications;

pub const CRATE_NAME: &str = "umux-notify";

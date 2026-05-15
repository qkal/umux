// SPDX-License-Identifier: GPL-3.0-or-later

use time::OffsetDateTime;

use crate::{OscNotificationKind, TerminalNotification};

const OSC_START: &str = "\u{1b}]";
const BEL: char = '\u{7}';
const ST: &str = "\u{1b}\\";

pub fn parse_osc_notifications(input: &str) -> Vec<TerminalNotification> {
    let mut notifications = Vec::new();
    let mut cursor = 0;

    while let Some(start) = input[cursor..].find(OSC_START) {
        let body_start = cursor + start + OSC_START.len();
        let Some((body_end, terminator_len)) = find_osc_terminator(&input[body_start..]) else {
            break;
        };

        let body = &input[body_start..body_start + body_end];
        if let Some(notification) = parse_osc_body(body) {
            notifications.push(notification);
        }

        cursor = body_start + body_end + terminator_len;
    }

    notifications
}

fn find_osc_terminator(input: &str) -> Option<(usize, usize)> {
    let bel = input.find(BEL).map(|index| (index, BEL.len_utf8()));
    let st = input.find(ST).map(|index| (index, ST.len()));

    match (bel, st) {
        (Some(bel), Some(st)) => Some(if bel.0 <= st.0 { bel } else { st }),
        (Some(bel), None) => Some(bel),
        (None, Some(st)) => Some(st),
        (None, None) => None,
    }
}

fn parse_osc_body(body: &str) -> Option<TerminalNotification> {
    let (kind, title, message) = match body.split_once(';')? {
        ("9", message) => (OscNotificationKind::Osc9, None, message),
        ("99", message) => (OscNotificationKind::Osc99, None, message),
        ("777", payload) => {
            let mut parts = payload.splitn(3, ';');
            match (parts.next(), parts.next(), parts.next()) {
                (Some("notify"), Some(title), Some(message)) => {
                    (OscNotificationKind::Osc777, Some(title), message)
                }
                _ => return None,
            }
        }
        _ => return None,
    };

    Some(TerminalNotification {
        kind,
        title: title.map(str::to_owned),
        message: message.to_owned(),
        received_at: OffsetDateTime::now_utc(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OscNotificationKind;

    #[test]
    fn parses_osc_9_bel_notification() {
        let notifications = parse_osc_notifications("before\u{1b}]9;build finished\u{7}after");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].kind, OscNotificationKind::Osc9);
        assert_eq!(notifications[0].message, "build finished");
    }

    #[test]
    fn parses_osc_99_st_notification() {
        let notifications = parse_osc_notifications("\u{1b}]99;agent: waiting\u{1b}\\");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].kind, OscNotificationKind::Osc99);
        assert_eq!(notifications[0].message, "agent: waiting");
    }

    #[test]
    fn parses_osc_777_notify_title_and_body() {
        let notifications = parse_osc_notifications("\u{1b}]777;notify;Codex;Needs input\u{7}");

        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].kind, OscNotificationKind::Osc777);
        assert_eq!(notifications[0].title.as_deref(), Some("Codex"));
        assert_eq!(notifications[0].message, "Needs input");
    }

    #[test]
    fn ignores_unrelated_osc_sequences() {
        let notifications = parse_osc_notifications("\u{1b}]0;window title\u{7}");

        assert!(notifications.is_empty());
    }
}

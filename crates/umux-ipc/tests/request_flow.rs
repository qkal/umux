// SPDX-License-Identifier: GPL-3.0-or-later

use serde_json::json;
use umux_ipc::{Method, RequestFrame, ResponseFrame};

#[test]
fn request_and_response_flow_is_newline_delimited_json() {
    let request = RequestFrame::new(42, Method::NotificationCreate, json!({"message":"ready"}));
    let line = request.to_json_line().unwrap();
    let decoded = RequestFrame::from_json_line(&line).unwrap();
    let response = ResponseFrame::ok(decoded.id, json!({"accepted":true}));

    assert_eq!(decoded.method, Method::NotificationCreate);
    assert_eq!(response.result().unwrap()["accepted"], true);
}

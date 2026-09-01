// SPDX-FileCopyrightText: 2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

//! run_stream 解码器公共 API 集成测试（不依赖真实服务，纯解码层）。
//!
//! 覆盖从 crate 外部消费 decode_sse_frame / RunStreamEnvelope /
//! RunStreamKind 的行为，验证公共 API 面与 SSoT 生成常量的一致性。

use agentrt_rs::{decode_sse_frame, RunStreamEnvelope, RunStreamKind};

#[test]
fn decode_public_api_run_start() {
    let line = r#"data: {"v":1,"type":"run_start","id":0,"session_id":"sess_test","ts":10,"epoch":0,"data":{"prompt":"hello","agent":"default"}}"#;
    let ev: RunStreamEnvelope = decode_sse_frame(line).expect("decode run_start via public API");
    assert_eq!(ev.protocol_v, 1);
    assert_eq!(ev.event_type, "run_start");
    assert_eq!(ev.kind(), RunStreamKind::Control);
    assert_eq!(ev.session_id.as_deref(), Some("sess_test"));
    assert_eq!(ev.data["prompt"], "hello");
}

#[test]
fn decode_public_api_ignore_keepalive() {
    assert!(decode_sse_frame(": keep-alive").is_none());
    assert!(decode_sse_frame("").is_none());
}

#[test]
fn decode_public_api_unknown_type() {
    let line = r#"data: {"v":1,"type":"future_type_x","id":1,"ts":1,"epoch":0,"data":{"a":1}}"#;
    let ev = decode_sse_frame(line).expect("future type decoded");
    assert_eq!(ev.kind(), RunStreamKind::Unknown);
}

#[test]
fn decode_public_api_multi_events_sequence() {
    /* 模拟真实 SSE 流：run_start → plan → token_delta → run_end */
    let frames = [
        r#"data: {"v":1,"type":"run_start","id":0,"session_id":"s","ts":1,"epoch":0,"data":{}}"#,
        r#"data: {"v":1,"type":"plan","id":1,"ts":2,"epoch":0,"data":{"steps":[]}}"#,
        r#"data: {"v":1,"type":"token_delta","id":2,"ts":3,"epoch":0,"data":{"delta":"hi"}}"#,
        r#"data: {"v":1,"type":"run_end","id":3,"ts":4,"epoch":0,"data":{"status":"completed"}}"#,
    ];
    let kinds: Vec<RunStreamKind> = frames
        .iter()
        .filter_map(|f| decode_sse_frame(f))
        .map(|ev| ev.kind())
        .collect();
    assert_eq!(
        kinds,
        vec![
            RunStreamKind::Control,
            RunStreamKind::Cognition,
            RunStreamKind::Outcome,
            RunStreamKind::Control,
        ]
    );
}

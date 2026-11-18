// SPDX-FileCopyrightText: 2025-2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

//! D-12：网关 503（服务重启中）必须归为「暂不可达」而非失败。
//!
//! 用本地裸 TCP 服务回放一帧真实 HTTP/1.1 应答（不 stub 客户端内部），
//! 验证 `post_stream` 的状态码分类与错误码语义：503 → CODE_UNAVAILABLE，
//! 其余 5xx 仍走 CODE_SERVER_ERROR，两条路径不混。

use agentrt_rs::*;
use serde_json::json;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

/// 起一个只应答一次的 HTTP 服务，返回 (endpoint, 线程句柄)。
fn serve_once(status: &'static str, body: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
    let addr = listener.local_addr().expect("local_addr");
    let handle = thread::spawn(move || {
        if let Ok((mut sock, _)) = listener.accept() {
            let mut buf = [0u8; 8192];
            let _ = sock.read(&mut buf);
            let resp = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = sock.write_all(resp.as_bytes());
        }
    });
    (format!("http://{addr}"), handle)
}

const UNAVAIL_BODY: &str = r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"agent service restarting","data":{"code":"UNAVAILABLE","retry_after_ms":900}}}"#;

#[tokio::test]
async fn post_stream_503_maps_to_unavailable() {
    let (endpoint, handle) = serve_once("503 Service Unavailable", UNAVAIL_BODY);
    let client = Client::new_with_timeout(&endpoint, std::time::Duration::from_secs(5))
        .expect("client");
    let err = client
        .post_stream("/v1/agent/stream", &json!({"method": "agent.run_stream"}))
        .await
        .expect_err("503 must be an error");
    assert_eq!(err.code(), CODE_UNAVAILABLE);
    assert!(err.is_unavailable(), "503 must classify as unavailable");
    assert_ne!(
        err.code(),
        CODE_SERVER_ERROR,
        "unavailable must not share the real-failure code"
    );
    assert!(
        err.to_string().contains("retry_after_ms"),
        "backend retry hint must survive to the caller: {err}"
    );
    handle.join().expect("server thread");
}

#[tokio::test]
async fn post_stream_500_stays_server_error() {
    let (endpoint, handle) = serve_once("500 Internal Server Error", r#"{"error":"boom"}"#);
    let client = Client::new_with_timeout(&endpoint, std::time::Duration::from_secs(5))
        .expect("client");
    let err = client
        .post_stream("/v1/agent/stream", &json!({}))
        .await
        .expect_err("500 must be an error");
    assert_eq!(err.code(), CODE_SERVER_ERROR);
    assert!(!err.is_unavailable(), "500 is a real failure");
    handle.join().expect("server thread");
}

// SPDX-FileCopyrightText: 2025-2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

//! D-20：svc_call 通用服务调用回归。用本地 AF_UNIX 假守护验证
//! 路由、请求帧与 result/error 分诊，不依赖真实 daemon。

#[cfg(unix)]
mod unix_tests {
    use agentrt_rs::syscall::SvcCall;
    use serde_json::{json, Value};
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn sock_path(tag: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("airy-svc-{tag}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("svc.sock")
    }

    fn serve_once(listener: UnixListener, reply: &'static str) -> std::thread::JoinHandle<Value> {
        std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().unwrap();
            let mut buf = [0u8; 4096];
            let n = conn.read(&mut buf).unwrap();
            let req: Value = serde_json::from_slice(&buf[..n]).unwrap();
            conn.write_all(reply.as_bytes()).unwrap();
            req
        })
    }

    fn client(sock: &PathBuf) -> SvcCall {
        SvcCall::with_sockets(sock.clone(), sock.clone(), sock.clone())
            .with_timeout(Duration::from_secs(5))
    }

    #[test]
    fn svc_call_routes_method_and_returns_result() {
        let sock = sock_path("ok");
        let listener = UnixListener::bind(&sock).unwrap();
        let server = serve_once(listener, r#"{"jsonrpc":"2.0","id":1,"result":{"hits":2}}"#);

        let got = client(&sock)
            .call("mem", "memory.search", json!({"query": "x"}))
            .unwrap();
        assert_eq!(got["hits"], 2);

        let req = server.join().unwrap();
        assert_eq!(req["jsonrpc"], "2.0");
        assert_eq!(req["method"], "memory.search");
        assert_eq!(req["params"]["query"], "x");
        assert_eq!(req["id"], 1);
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }

    #[test]
    fn svc_call_surfaces_jsonrpc_error_without_retry() {
        let sock = sock_path("err");
        let listener = UnixListener::bind(&sock).unwrap();
        let server = serve_once(
            listener,
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32601,"message":"method not found"}}"#,
        );

        let err = client(&sock)
            .call("tool", "tool.nope", json!({}))
            .unwrap_err();
        assert!(err.to_string().contains("method not found"), "err={err}");
        let _ = server.join().unwrap();
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }

    #[test]
    fn svc_call_rejects_unknown_daemon() {
        let sock = sock_path("bad");
        let err = client(&sock).call("kernel", "x", json!({})).unwrap_err();
        assert!(err.to_string().contains("Invalid daemon"), "err={err}");
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }

    #[test]
    fn svc_call_rejects_empty_method() {
        let sock = sock_path("empty");
        let err = client(&sock).call("agent", "  ", json!({})).unwrap_err();
        assert!(err.to_string().contains("Invalid method"), "err={err}");
        let _ = std::fs::remove_dir_all(sock.parent().unwrap());
    }
}

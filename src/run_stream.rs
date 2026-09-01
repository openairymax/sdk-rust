// SPDX-FileCopyrightText: 2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

//! @file run_stream.rs
//! @brief agent.run_stream SSE 事件解码器（M1-1d §2.4 v1 信封协议）。
//!
//! SSoT：信封字段键 / 事件类型常量由 build.rs 从 C 侧唯一权威
//! airy_run_stream.h 生成（run_stream_gen 模块），本模块只做结构化
//! 解析，禁止手写 wire 字符串字面量。
//!
//! 宽容读取（方案 §2.4.4）：未知事件类型一律归类 RunStreamKind::Unknown
//! 不崩溃；未知字段透传；v/type 缺失视为无效帧返回 None。

include!(concat!(env!("OUT_DIR"), "/run_stream_gen.rs"));

use crate::Client;
use crate::error::AgentOSError;
use serde_json::Value;

/// 事件信封（wire JSON 外层，见方案 §2.4.2）
#[derive(Debug, Clone, PartialEq)]
pub struct RunStreamEnvelope {
    /// 协议版本（v）
    pub protocol_v: i64,
    /// 事件类型（type，见方案 §2.4.3 分层枚举）
    pub event_type: String,
    /// 帧序列号：同一 run 内单调递增，用于去重/对齐/重播
    pub id: i64,
    /// 本次运行唯一标识
    pub run_id: Option<String>,
    /// 会话标识（sess_ 前缀，与 agent.cancel 对齐）
    pub session_id: Option<String>,
    /// 单调毫秒时间戳
    pub ts: i64,
    /// 策略/版本 epoch（M2 后用于 PEP 相关帧失效标记；预留）
    pub epoch: i64,
    /// 类型化负载（data，键名见 C 头各事件类型段）
    pub data: Value,
}

/// 事件分层（方案 §2.4.3）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunStreamKind {
    Control,
    Cognition,
    Execution,
    Outcome,
    Unknown,
}

impl RunStreamEnvelope {
    /// 从已解析的 JSON 值构造信封（键名全部来自 SSoT 生成常量）
    pub fn from_value(v: &Value) -> Option<Self> {
        let protocol_v = v.get(gen::AIRY_RS_K_V)?.as_i64()?;
        let event_type = v.get(gen::AIRY_RS_K_TYPE)?.as_str()?.to_string();
        Some(Self {
            protocol_v,
            event_type,
            id: v.get(gen::AIRY_RS_K_ID).and_then(Value::as_i64).unwrap_or(0),
            run_id: v
                .get(gen::AIRY_RS_K_RUN_ID)
                .and_then(Value::as_str)
                .map(String::from),
            session_id: v
                .get(gen::AIRY_RS_K_SESSION)
                .and_then(Value::as_str)
                .map(String::from),
            ts: v.get(gen::AIRY_RS_K_TS).and_then(Value::as_i64).unwrap_or(0),
            epoch: v.get(gen::AIRY_RS_K_EPOCH).and_then(Value::as_i64).unwrap_or(0),
            data: v.get(gen::AIRY_RS_K_DATA).cloned().unwrap_or(Value::Null),
        })
    }

    /// 事件分层分类；未知类型返回 Unknown（宽容读取）
    pub fn kind(&self) -> RunStreamKind {
        match self.event_type.as_str() {
            gen::AIRY_RS_TYPE_RUN_START
            | gen::AIRY_RS_TYPE_RUN_END
            | gen::AIRY_RS_TYPE_ERROR => RunStreamKind::Control,
            gen::AIRY_RS_TYPE_PLAN => RunStreamKind::Cognition,
            gen::AIRY_RS_TYPE_TOOL_START
            | gen::AIRY_RS_TYPE_TOOL_END
            | gen::AIRY_RS_TYPE_TOOL_DELTA => RunStreamKind::Execution,
            gen::AIRY_RS_TYPE_TOKEN_DELTA | gen::AIRY_RS_TYPE_MESSAGE => RunStreamKind::Outcome,
            _ => RunStreamKind::Unknown,
        }
    }
}

/// 解析单条 SSE 行（"data: <json>"），返回事件信封；非 data 行 / 无效帧
/// 返回 None（SSE 注释帧、空行、其他事件名均忽略）。
pub fn decode_sse_frame(line: &str) -> Option<RunStreamEnvelope> {
    let line = line.trim();
    if line.is_empty() || line.starts_with(':') {
        return None;
    }
    let rest = line.strip_prefix("data:")?.trim();
    let v: Value = serde_json::from_str(rest).ok()?;
    RunStreamEnvelope::from_value(&v)
}

/// 订阅 agent.run_stream SSE 事件流（gateway 纯翻译端点）。
///
/// 每收到一个事件帧即调用 on_event；流结束或出错返回。调用方可按
/// event_type 分发处理（宽容读取：未知类型以 Unknown 呈现）。
pub async fn run_stream_events<F>(
    client: &Client,
    params: Value,
    mut on_event: F,
) -> Result<(), AgentOSError>
where
    F: FnMut(RunStreamEnvelope),
{
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "agent.run_stream",
        "params": params,
    });
    let resp = client.post_stream("/api/v1/agent/run/stream", &body).await?;

    use futures::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut pending: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| AgentOSError::network(&e.to_string()))?;
        pending.extend_from_slice(&chunk);
        loop {
            match pending.iter().position(|&b| b == b'\n') {
                Some(pos) => {
                    let line: Vec<u8> = pending.drain(..=pos).collect();
                    if let Some(ev) = decode_sse_frame(&String::from_utf8_lossy(&line)) {
                        on_event(ev);
                    }
                }
                None => break,
            }
        }
    }
    /* 尾部残行（无 \n 收尾） */
    if !pending.is_empty() {
        if let Some(ev) = decode_sse_frame(&String::from_utf8_lossy(&pending)) {
            on_event(ev);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /* ---- SSoT 门禁：生成常量必须与 C 头语义一致 ---- */

    #[test]
    fn test_gen_version() {
        assert_eq!(gen::AIRY_RS_VERSION, 1);
    }

    #[test]
    fn test_gen_envelope_keys() {
        assert_eq!(gen::AIRY_RS_K_V, "v");
        assert_eq!(gen::AIRY_RS_K_TYPE, "type");
        assert_eq!(gen::AIRY_RS_K_ID, "id");
        assert_eq!(gen::AIRY_RS_K_RUN_ID, "run_id");
        assert_eq!(gen::AIRY_RS_K_SESSION, "session_id");
        assert_eq!(gen::AIRY_RS_K_TS, "ts");
        assert_eq!(gen::AIRY_RS_K_EPOCH, "epoch");
        assert_eq!(gen::AIRY_RS_K_DATA, "data");
    }

    #[test]
    fn test_gen_event_types() {
        assert_eq!(gen::AIRY_RS_TYPE_RUN_START, "run_start");
        assert_eq!(gen::AIRY_RS_TYPE_RUN_END, "run_end");
        assert_eq!(gen::AIRY_RS_TYPE_ERROR, "error");
        assert_eq!(gen::AIRY_RS_TYPE_PLAN, "plan");
        assert_eq!(gen::AIRY_RS_TYPE_TOOL_START, "tool_start");
        assert_eq!(gen::AIRY_RS_TYPE_TOOL_END, "tool_end");
        assert_eq!(gen::AIRY_RS_TYPE_TOOL_DELTA, "tool_delta");
        assert_eq!(gen::AIRY_RS_TYPE_TOKEN_DELTA, "token_delta");
        assert_eq!(gen::AIRY_RS_TYPE_MESSAGE, "message");
    }

    /* ---- 解码路径 ---- */

    #[test]
    fn test_decode_run_start() {
        let line = r#"data: {"v":1,"type":"run_start","id":0,"session_id":"sess_x","ts":1,"epoch":0,"data":{"prompt":"hi"}}"#;
        let ev = decode_sse_frame(line).expect("decode run_start");
        assert_eq!(ev.protocol_v, 1);
        assert_eq!(ev.event_type, gen::AIRY_RS_TYPE_RUN_START);
        assert_eq!(ev.session_id.as_deref(), Some("sess_x"));
        assert_eq!(ev.data[gen::AIRY_RS_K_PROMPT], "hi");
        assert_eq!(ev.kind(), RunStreamKind::Control);
    }

    #[test]
    fn test_decode_with_trailing_crlf() {
        let line = "data: {\"v\":1,\"type\":\"run_end\",\"id\":1,\"ts\":2,\"epoch\":0,\"data\":{\"status\":\"completed\"}}\r\n";
        let ev = decode_sse_frame(line).expect("decode run_end");
        assert_eq!(ev.event_type, gen::AIRY_RS_TYPE_RUN_END);
        assert_eq!(ev.kind(), RunStreamKind::Control);
        assert_eq!(ev.data[gen::AIRY_RS_K_STATUS], "completed");
    }

    #[test]
    fn test_decode_plan_and_execution() {
        let line = r#"data: {"v":1,"type":"plan","id":2,"ts":3,"epoch":0,"data":{"plan":[],"steps":[]}}"#;
        assert_eq!(decode_sse_frame(line).unwrap().kind(), RunStreamKind::Cognition);

        let line = r#"data: {"v":1,"type":"tool_start","id":3,"ts":4,"epoch":0,"data":{"tool":"web_search","tool_id":"t1"}}"#;
        assert_eq!(decode_sse_frame(line).unwrap().kind(), RunStreamKind::Execution);

        let line = r#"data: {"v":1,"type":"message","id":5,"ts":6,"epoch":0,"data":{"role":"assistant","content":"ok"}}"#;
        assert_eq!(decode_sse_frame(line).unwrap().kind(), RunStreamKind::Outcome);
    }

    #[test]
    fn test_decode_unknown_type_tolerated() {
        /* §2.4.4：未来新增类型，旧客户端必须忽略不崩 */
        let line = r#"data: {"v":1,"type":"stream_event","id":9,"ts":7,"epoch":0,"data":{"mime":"audio/wav"}}"#;
        let ev = decode_sse_frame(line).expect("unknown type tolerated");
        assert_eq!(ev.event_type, "stream_event");
        assert_eq!(ev.kind(), RunStreamKind::Unknown);
    }

    #[test]
    fn test_decode_non_data_lines() {
        assert!(decode_sse_frame("").is_none());
        assert!(decode_sse_frame(": keep-alive").is_none());
        assert!(decode_sse_frame("event: foo").is_none());
        assert!(decode_sse_frame("data: not-json").is_none());
    }

    #[test]
    fn test_decode_missing_core_fields() {
        /* v/type 缺失视为无效帧 */
        assert!(decode_sse_frame(r#"data: {"id":1}"#).is_none());
        assert!(decode_sse_frame(r#"data: {"v":1}"#).is_none());
    }

    #[test]
    fn test_decode_missing_optional_fields_defaulted() {
        /* id/ts/epoch/run_id/session_id 缺失宽容默认，不拒绝 */
        let line = r#"data: {"v":1,"type":"run_start","data":{}}"#;
        let ev = decode_sse_frame(line).expect("optional fields defaulted");
        assert_eq!(ev.id, 0);
        assert_eq!(ev.ts, 0);
        assert_eq!(ev.epoch, 0);
        assert_eq!(ev.run_id, None);
    }
}

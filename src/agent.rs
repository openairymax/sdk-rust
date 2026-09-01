// SPDX-FileCopyrightText: 2025-2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

// AgentRT Rust SDK Agent
// Version: 0.1.1
// Last updated: 2026-03-23

use crate::Client;

/// AgentRT 代理入口
#[derive(Debug, Clone)]
pub struct Agent {
    client: Client,
}

impl Agent {
    /// 创建新的 AgentRT 代理
    pub fn new(client: Client) -> Self {
        Agent { client }
    }

    /// 获取底层客户端引用
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// 获取客户端克隆
    pub fn into_client(self) -> Client {
        self.client
    }

    /// 健康检查
    pub async fn health(&self) -> bool {
        self.client.health().await.is_ok()
    }

    /// 获取端点地址
    pub fn endpoint(&self) -> &str {
        self.client.endpoint()
    }

    /// 流式运行 agent（agent.run_stream SSE 订阅，M1-1d §2.4）
    ///
    /// # 参数
    /// - `params`: 运行参数（prompt 等，原样透传给引擎）
    /// - `on_event`: 每收到一个事件帧的回调（宽容读取，未知类型不崩）
    ///
    /// # 返回
    /// 流结束或出错时返回；错误信封（type=error）以事件帧形式回调，
    /// 由调用方决策是否中断。
    pub async fn run_stream<F>(
        &self,
        params: serde_json::Value,
        on_event: F,
    ) -> Result<(), crate::error::AgentOSError>
    where
        F: FnMut(crate::run_stream::RunStreamEnvelope),
    {
        crate::run_stream::run_stream_events(&self.client, params, on_event).await
    }
}

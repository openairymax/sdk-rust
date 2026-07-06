**语言:** [English](README.md) | 简体中文

# Airymax Rust SDK

[![Version](https://img.shields.io/badge/version-0.1.1-5a6b7e)](https://atomgit.com/openairymax/sdk-rust)
[![License](https://img.shields.io/badge/license-AGPL--3.0+Apache--2.0-4a90d9)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)

> [Airymax](https://atomgit.com/openairymax/airymaxhub) AI 智能体运行时平台的官方 Rust 开发工具包。
> [sdk](https://atomgit.com/openairymax/sdk) 管理仓聚合的叶子仓之一。
> 同时也是 Airymax `cli` 和 `tui` 工具的基础 crate。

---

## 概述

**Airymax Rust SDK**（`agentrt-rs`，crate 名 `agentrt_rs`）提供内存安全、零成本抽象的 Airymax 运行时接口。它是面向性能与安全敏感场景的 SDK 首选，且与 Airymax `cli`、`tui` 工具共用同一 crate，因此此处新增的能力会立即在两个命令行界面中可用。

基于该 SDK 构建的 Agent 应用是**运行时租户**：通过 SDK 调用系统能力，而非直接访问内核内部。该 crate 以异步（Tokio）为核心、完整 `serde` 类型化，并暴露与运行时响应码一一对应的类型化错误码体系。

## 双层 API 架构

每个 Airymax SDK 都提供顶层 `AgentRTClient`，内嵌四个资源客户端，分别覆盖运行时的一个平面：

```
AgentRTClient
├── CognitionClient   # 认知平面：任务 / 循环 / 推理
├── SafetyClient      # 安全平面：审计 / 沙箱 / 策略
├── ToolClient        # 工具平面：注册 / 调用 / 编排
└── ChatClient        # 对话平面：LLM 路由 / 会话 / 流式
```

在 Rust 中通过 `client.cognition()`、`client.safety()`、`client.tool()`、`client.chat()` 访问，返回由共享、带连接池的 `reqwest` 传输支撑的借用客户端。

## 目录结构

```
sdk-rust/
├── src/
│   ├── lib.rs                  # crate 根、公共 API 导出、prelude
│   ├── agent.rs                # AgentRTClient 与内嵌资源客户端
│   ├── client/
│   │   ├── mod.rs              # 客户端模块入口
│   │   └── client.rs           # Client / APIClient 实现
│   ├── modules/                # 业务模块管理器
│   │   ├── mod.rs
│   │   ├── task/               # TaskManager
│   │   ├── memory/             # MemoryManager
│   │   ├── session/            # SessionManager
│   │   └── skill/              # SkillManager
│   ├── hook.rs                 # Hook 系统
│   ├── plugin.rs               # 插件系统
│   ├── telemetry.rs            # OpenTelemetry 追踪
│   ├── syscall.rs              # 系统调用绑定
│   ├── protocol.rs             # 协议处理
│   ├── error.rs                # AgentOSError 与错误码常量
│   ├── macros.rs               # 辅助宏
│   ├── types/                  # 枚举与领域模型
│   ├── utils/                  # 工具函数
│   └── task.rs / memory.rs / session.rs / skill.rs  # 旧版重导出
├── tests/                      # 集成测试（管理器、异步、基准）
├── Cargo.toml                  # crate 清单（agentrt-rs）
└── README.md                   # 本文件
```

## 上下游依赖

### 上游

- **运行时**：通过 HTTP 和 JSON-RPC 2.0 连接到运行中的 Airymax / AgentRT 实例（`gateway_d`）。
- **协议**：使用平台 `protocols/` 中定义的 AgentsIPC 协议。
- **配置**：依次从构造参数、环境变量（`AGENTRT_ENDPOINT`、`AGENTRT_TIMEOUT`、`AGENTRT_API_KEY`）、默认值 `http://127.0.0.1:18789` 解析。

### 下游

- **Agent 应用**：用户编写的 Agent 依赖 `agentrt-rs` 成为运行时租户。
- **CLI / TUI 工具**：Airymax 的 `cli` 和 `tui` crate 直接消费本 SDK 与运行时通信。
- **示例**：平台 `ecosystem/examples/` 中的参考 Agent。

## 安装

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
agentrt-rs = "0.1.1"
tokio = { version = "1", features = ["full"] }
```

**环境要求：** Rust edition 2021（stable 工具链）。运行时依赖：`reqwest`（HTTP）、`tokio`（异步运行时）、`serde` / `serde_json`（序列化）、`thiserror` / `anyhow`（错误）、`async-trait`、`chrono`、`uuid`、`urlencoding`、`rand`、`once_cell`、`futures`。

## 快速入门

```rust
use agentrt_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentRTClient::new("http://localhost:18789");

    // 认知平面 —— 提交并等待任务
    let task = client.cognition().submit_task(r#"{"input": "analyze data"}"#).await?;
    let result = client.cognition().wait(&task.id, std::time::Duration::from_secs(60)).await?;
    println!("Task result: {:?}", result.output);

    // 工具平面 —— 调用已注册工具
    let res = client.tool().invoke("web-scraper", serde_json::json!({"url": "https://example.com"})).await?;

    Ok(())
}
```

### 认证与配置

```rust
use agentrt_rs::{AgentRTClient, ClientConfig};

let config = ClientConfig::builder()
    .endpoint("http://localhost:18789")
    .timeout(std::time::Duration::from_secs(30))
    .max_retries(3)
    .api_key("your-api-key")
    .build()?;

let client = AgentRTClient::with_config(config);
```

### 模块管理器（底层 API）

如需更细粒度控制，SDK 还暴露了支撑四个平面的底层管理器（`TaskManager`、`MemoryManager`、`SessionManager`、`SkillManager`）。

```rust
use std::sync::Arc;
use agentrt_rs::{AgentRTClient, MemoryLayer};
use agentrt_rs::modules::memory::MemoryManager;

let client = AgentRTClient::new("http://localhost:18789");
let memory_mgr = MemoryManager::new(Arc::new(client));
let memory_id = memory_mgr.write("content", MemoryLayer::L1).await?;
let hits = memory_mgr.search("query", 5).await?;
```

## 构建与测试

```bash
# 构建
cargo build --release

# 运行全部测试
cargo test

# 运行指定集成测试
cargo test --test managers_test

# Lint
cargo clippy --all-targets

# 生成文档
cargo doc --open
```

## 分支策略

本叶子仓在 **`feature/official-hubs-01`** 分支上开发。聚合管理仓 `sdk` 仅使用 `main` 分支。

## 许可证

采用 **AGPL v3 + Apache 2.0** 双许可证（SPDX: `AGPL-3.0-or-later OR Apache-2.0`）。详见 [LICENSE](LICENSE)。

Copyright (c) 2025-2026 **SPHARX Ltd.** All Rights Reserved.

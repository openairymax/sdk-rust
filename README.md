**Language:** English | [简体中文](README_zh.md)

# Airymax Rust SDK

[![Version](https://img.shields.io/badge/version-0.1.1-5a6b7e)](https://atomgit.com/openairymax/sdk-rust)
[![License](https://img.shields.io/badge/license-AGPL--3.0+Apache--2.0-4a90d9)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)

> Official Rust development kit for the [Airymax](https://atomgit.com/openairymax/airymaxhub) AI Agent Runtime Platform.
> One of the leaf repositories aggregated by the [sdk](https://atomgit.com/openairymax/sdk) management repo.
> Also serves as the foundation crate for the Airymax `cli` and `tui` tools.

---

## Overview

The **Airymax Rust SDK** (`agentrt-rs`, crate `agentrt_rs`) provides a memory-safe, zero-cost-abstraction interface to the Airymax runtime. It is the SDK of choice for performance- and safety-critical workloads, and it is the same crate that powers the Airymax `cli` and `tui` tools, so any feature added here is immediately available to both command-line surfaces.

Agent applications built on this SDK are **runtime tenants**: they invoke system capabilities through the SDK rather than touching kernel internals directly. The crate is async-first (Tokio), fully `serde`-typed, and exposes a typed error-code system that mirrors the runtime's response codes.

## Double-Layer API Architecture

Every Airymax SDK ships a top-level `AgentRTClient` that nests four resource clients, each covering one plane of the runtime:

```
AgentRTClient
├── CognitionClient   # Cognition plane: tasks / loops / inference
├── SafetyClient      # Safety plane: audit / sandbox / policy
├── ToolClient        # Tool plane: register / invoke / orchestrate
└── ChatClient        # Chat plane: LLM routing / sessions / streaming
```

In Rust these are accessed through `client.cognition()`, `client.safety()`, `client.tool()`, and `client.chat()`, returning borrowed clients backed by a shared, connection-pooled `reqwest` transport.

## Directory Structure

```
sdk-rust/
├── src/
│   ├── lib.rs                  # Crate root, public API exports, prelude
│   ├── agent.rs                # AgentRTClient + nested resource clients
│   ├── client/
│   │   ├── mod.rs              # Client module entry
│   │   └── client.rs           # Client / APIClient implementation
│   ├── modules/                # Domain module managers
│   │   ├── mod.rs
│   │   ├── task/               # TaskManager
│   │   ├── memory/             # MemoryManager
│   │   ├── session/            # SessionManager
│   │   └── skill/              # SkillManager
│   ├── hook.rs                 # Hook system
│   ├── plugin.rs               # Plugin system
│   ├── telemetry.rs            # OpenTelemetry tracing
│   ├── syscall.rs              # Syscall bindings
│   ├── protocol.rs             # Protocol handling
│   ├── error.rs                # AgentOSError + error-code constants
│   ├── macros.rs               # Helper macros
│   ├── types/                  # Enums + domain models
│   ├── utils/                  # Helpers
│   └── task.rs / memory.rs / session.rs / skill.rs  # Legacy re-exports
├── tests/                      # Integration tests (managers, async, benchmarks)
├── Cargo.toml                  # Crate manifest (agentrt-rs)
└── README.md                   # This file
```

## Upstream & Downstream Dependencies

### Upstream

- **Runtime**: Connects to a running Airymax / AgentRT instance (`gateway_d`) over HTTP and JSON-RPC 2.0.
- **Protocol**: Speaks the AgentsIPC protocol defined in the platform `protocols/` tree.
- **Configuration**: Resolved from constructor arguments, then environment variables (`AGENTRT_ENDPOINT`, `AGENTRT_TIMEOUT`, `AGENTRT_API_KEY`), then a `http://127.0.0.1:18789` default.

### Downstream

- **Agent applications**: User-written agents depend on `agentrt-rs` to become runtime tenants.
- **CLI / TUI tools**: The Airymax `cli` and `tui` crates consume this SDK directly to talk to the runtime.
- **Examples**: Reference agents in the platform `ecosystem/examples/`.

## Installation

Add the crate to your `Cargo.toml`:

```toml
[dependencies]
agentrt-rs = "0.1.1"
tokio = { version = "1", features = ["full"] }
```

**Requirements:** Rust edition 2021 (stable toolchain). Runtime dependencies: `reqwest` (HTTP), `tokio` (async runtime), `serde` / `serde_json` (serialization), `thiserror` / `anyhow` (errors), `async-trait`, `chrono`, `uuid`, `urlencoding`, `rand`, `once_cell`, `futures`.

## Quick Start

```rust
use agentrt_rs::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = AgentRTClient::new("http://localhost:18789");

    // Cognition plane — submit and await a task
    let task = client.cognition().submit_task(r#"{"input": "analyze data"}"#).await?;
    let result = client.cognition().wait(&task.id, std::time::Duration::from_secs(60)).await?;
    println!("Task result: {:?}", result.output);

    // Tool plane — invoke a registered tool
    let res = client.tool().invoke("web-scraper", serde_json::json!({"url": "https://example.com"})).await?;

    Ok(())
}
```

### Authentication & configuration

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

### Module managers (lower-level API)

For fine-grained control the SDK also exposes the underlying managers (`TaskManager`, `MemoryManager`, `SessionManager`, `SkillManager`) that back the four planes.

```rust
use std::sync::Arc;
use agentrt_rs::{AgentRTClient, MemoryLayer};
use agentrt_rs::modules::memory::MemoryManager;

let client = AgentRTClient::new("http://localhost:18789");
let memory_mgr = MemoryManager::new(Arc::new(client));
let memory_id = memory_mgr.write("content", MemoryLayer::L1).await?;
let hits = memory_mgr.search("query", 5).await?;
```

## Build & Test

```bash
# Build
cargo build --release

# Run all tests
cargo test

# Run a specific integration test
cargo test --test managers_test

# Lint
cargo clippy --all-targets

# Generate docs
cargo doc --open
```

## Branch Strategy

This leaf repository is developed on **`feature/official-hubs-01`**. The aggregating `sdk` management repo stays on `main`.

## License

Dual-licensed under **AGPL v3 + Apache 2.0** (SPDX: `AGPL-3.0-or-later OR Apache-2.0`). See [LICENSE](LICENSE) for the full text.

Copyright (c) 2025-2026 **SPHARX Ltd.** All Rights Reserved.

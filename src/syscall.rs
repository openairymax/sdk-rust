// SPDX-FileCopyrightText: 2025-2026 SPHARX Ltd.
// SPDX-License-Identifier: AGPL-3.0-or-later OR Apache-2.0

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
#[cfg(unix)]
use std::io::{Read, Write};
use std::path::PathBuf;
#[cfg(unix)]
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::AgentOSError;

type Result<T> = std::result::Result<T, AgentOSError>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyscallNamespace {
    #[serde(rename = "task")]
    Task,
    #[serde(rename = "memory")]
    Memory,
    #[serde(rename = "session")]
    Session,
    #[serde(rename = "skill")]
    Skill,
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "telemetry")]
    Telemetry,
}

impl std::fmt::Display for SyscallNamespace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyscallNamespace::Task => write!(f, "task"),
            SyscallNamespace::Memory => write!(f, "memory"),
            SyscallNamespace::Session => write!(f, "session"),
            SyscallNamespace::Skill => write!(f, "skill"),
            SyscallNamespace::Agent => write!(f, "agent"),
            SyscallNamespace::Telemetry => write!(f, "telemetry"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallRequest {
    pub namespace: SyscallNamespace,
    pub operation: String,
    pub params: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyscallResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

pub trait SyscallBinding {
    fn invoke(&self, request: SyscallRequest) -> Result<SyscallResponse>;
}

pub struct HttpSyscallBinding {
    base_url: String,
    client: reqwest::blocking::Client,
}

impl HttpSyscallBinding {
    pub fn new(base_url: &str) -> Self {
        HttpSyscallBinding {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::blocking::Client::new(),
        }
    }

    pub fn with_timeout(base_url: &str, timeout: std::time::Duration) -> Self {
        HttpSyscallBinding {
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::blocking::Client::builder()
                .timeout(timeout)
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
        }
    }
}

impl SyscallBinding for HttpSyscallBinding {
    fn invoke(&self, request: SyscallRequest) -> Result<SyscallResponse> {
        let url = format!("{}/api/v1/syscall/{}", self.base_url, request.namespace);
        let body = serde_json::to_value(&request)
            .map_err(|e| AgentOSError::json(&e.to_string()))?;

        let resp = self.client
            .post(&url)
            .json(&body)
            .send()
            .map_err(|e| AgentOSError::network(&e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(AgentOSError::http(&format!("Syscall HTTP {}: {}", status, request.operation)));
        }

        let syscall_resp: SyscallResponse = resp
            .json()
            .map_err(|e| AgentOSError::json(&e.to_string()))?;

        Ok(syscall_resp)
    }
}

pub struct TaskSyscall<B: SyscallBinding> {
    binding: B,
}

impl<B: SyscallBinding> TaskSyscall<B> {
    pub fn new(binding: B) -> Self {
        TaskSyscall { binding }
    }

    pub fn submit(&self, description: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("description".to_string(), Value::String(description.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Task,
            operation: "submit".to_string(),
            params,
        })
    }

    pub fn query(&self, task_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("task_id".to_string(), Value::String(task_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Task,
            operation: "query".to_string(),
            params,
        })
    }

    pub fn wait(&self, task_id: &str, timeout_ms: u32) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("task_id".to_string(), Value::String(task_id.to_string()));
        params.insert("timeout_ms".to_string(), Value::Number(timeout_ms.into()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Task,
            operation: "wait".to_string(),
            params,
        })
    }

    pub fn cancel(&self, task_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("task_id".to_string(), Value::String(task_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Task,
            operation: "cancel".to_string(),
            params,
        })
    }
}

pub struct MemorySyscall<B: SyscallBinding> {
    binding: B,
}

impl<B: SyscallBinding> MemorySyscall<B> {
    pub fn new(binding: B) -> Self {
        MemorySyscall { binding }
    }

    pub fn write(&self, content: &str, metadata: Option<Value>) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("content".to_string(), Value::String(content.to_string()));
        if let Some(meta) = metadata {
            params.insert("metadata".to_string(), meta);
        }
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Memory,
            operation: "write".to_string(),
            params,
        })
    }

    pub fn read(&self, memory_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("memory_id".to_string(), Value::String(memory_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Memory,
            operation: "read".to_string(),
            params,
        })
    }

    pub fn get(&self, memory_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("memory_id".to_string(), Value::String(memory_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Memory,
            operation: "get".to_string(),
            params,
        })
    }

    pub fn search(&self, query: &str, top_k: u32) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("query".to_string(), Value::String(query.to_string()));
        params.insert("top_k".to_string(), Value::Number(top_k.into()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Memory,
            operation: "search".to_string(),
            params,
        })
    }

    pub fn delete(&self, memory_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("memory_id".to_string(), Value::String(memory_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Memory,
            operation: "delete".to_string(),
            params,
        })
    }
}

pub struct SessionSyscall<B: SyscallBinding> {
    binding: B,
}

impl<B: SyscallBinding> SessionSyscall<B> {
    pub fn new(binding: B) -> Self {
        SessionSyscall { binding }
    }

    pub fn create(&self) -> Result<SyscallResponse> {
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "create".to_string(),
            params: HashMap::new(),
        })
    }

    pub fn get(&self, session_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("session_id".to_string(), Value::String(session_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "get".to_string(),
            params,
        })
    }

    pub fn close(&self, session_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("session_id".to_string(), Value::String(session_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "close".to_string(),
            params,
        })
    }

    pub fn list(&self) -> Result<SyscallResponse> {
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "list".to_string(),
            params: HashMap::new(),
        })
    }

    pub fn set_context(&self, session_id: &str, key: &str, value: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("session_id".to_string(), Value::String(session_id.to_string()));
        params.insert("key".to_string(), Value::String(key.to_string()));
        params.insert("value".to_string(), Value::String(value.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "set_context".to_string(),
            params,
        })
    }

    pub fn get_context(&self, session_id: &str, key: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("session_id".to_string(), Value::String(session_id.to_string()));
        params.insert("key".to_string(), Value::String(key.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Session,
            operation: "get_context".to_string(),
            params,
        })
    }
}

pub struct SkillSyscall<B: SyscallBinding> {
    binding: B,
}

impl<B: SyscallBinding> SkillSyscall<B> {
    pub fn new(binding: B) -> Self {
        SkillSyscall { binding }
    }

    pub fn load(&self, skill_name: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("skill_name".to_string(), Value::String(skill_name.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Skill,
            operation: "load".to_string(),
            params,
        })
    }

    pub fn execute(&self, skill_id: &str, params_value: Value) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("skill_id".to_string(), Value::String(skill_id.to_string()));
        params.insert("params".to_string(), params_value);
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Skill,
            operation: "execute".to_string(),
            params,
        })
    }

    pub fn unload(&self, skill_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("skill_id".to_string(), Value::String(skill_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Skill,
            operation: "unload".to_string(),
            params,
        })
    }

    pub fn list(&self) -> Result<SyscallResponse> {
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Skill,
            operation: "list".to_string(),
            params: HashMap::new(),
        })
    }
}

pub struct AgentSyscall<B: SyscallBinding> {
    binding: B,
}

impl<B: SyscallBinding> AgentSyscall<B> {
    pub fn new(binding: B) -> Self {
        AgentSyscall { binding }
    }

    pub fn spawn(&self, spec: Value) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("spec".to_string(), spec);
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Agent,
            operation: "spawn".to_string(),
            params,
        })
    }

    pub fn terminate(&self, agent_id: &str) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("agent_id".to_string(), Value::String(agent_id.to_string()));
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Agent,
            operation: "terminate".to_string(),
            params,
        })
    }

    pub fn invoke(&self, agent_id: &str, method: &str, args: Value) -> Result<SyscallResponse> {
        let mut params = HashMap::new();
        params.insert("agent_id".to_string(), Value::String(agent_id.to_string()));
        params.insert("method".to_string(), Value::String(method.to_string()));
        params.insert("args".to_string(), args);
        self.binding.invoke(SyscallRequest {
            namespace: SyscallNamespace::Agent,
            operation: "invoke".to_string(),
            params,
        })
    }
}

// ── D-20 通用服务调用（svc_call）──────────────────────────────────
// 与 Python SDK `_IpcBackend.svc_call` 同语义：按守护进程名路由，方法名与
// 参数原样下发，result 原样返回；分域封装覆盖不到的已注册方法由此直达。
// 传输层仍是 AF_UNIX + JSON-RPC 2.0，与 daemon_rpc_client.c 一致。

const SVC_TIMEOUT: Duration = Duration::from_secs(30);
const SVC_RETRIES: usize = 3;
const SVC_BACKOFF_MS: u64 = 50;

/// 通用守护进程调用客户端。socket 路径解析顺序与 Python/C 侧一致：
/// 显式环境变量 > `AIRY_RUNTIME_DIR` > `$AIRY_HOME/run` > `~/.airymaxrt/run`。
pub struct SvcCall {
    mem_socket: PathBuf,
    agent_socket: PathBuf,
    tool_socket: PathBuf,
    timeout: Duration,
}

impl SvcCall {
    pub fn new() -> Self {
        Self::from_env()
    }

    pub fn from_env() -> Self {
        let run = runtime_dir();
        let socket = |env_key: &str, name: &str| -> PathBuf {
            std::env::var(env_key)
                .ok()
                .filter(|v| !v.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| run.join(name))
        };
        SvcCall {
            mem_socket: socket("AIRY_MEM_D_SOCKET", "mem.sock"),
            agent_socket: socket("AIRY_AGENT_D_SOCKET", "agent.sock"),
            tool_socket: socket("AIRY_TOOL_SOCK", "tool.sock"),
            timeout: SVC_TIMEOUT,
        }
    }

    pub fn with_sockets(mem: PathBuf, agent: PathBuf, tool: PathBuf) -> Self {
        SvcCall {
            mem_socket: mem,
            agent_socket: agent,
            tool_socket: tool,
            timeout: SVC_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// 按 daemon 名（mem / agent / tool）调用 method，params 必须是 JSON 对象。
    pub fn call(&self, daemon: &str, method: &str, params: Value) -> Result<Value> {
        let path = match daemon {
            "mem" => &self.mem_socket,
            "agent" => &self.agent_socket,
            "tool" => &self.tool_socket,
            other => {
                return Err(AgentOSError::config(&format!(
                    "Invalid daemon: {other:?} (expected mem/agent/tool)"
                )))
            }
        };
        if method.trim().is_empty() {
            return Err(AgentOSError::config("Invalid method"));
        }
        let params = if params.is_object() {
            params
        } else {
            serde_json::json!({})
        };
        self.call_socket(path, method, params)
    }

    #[cfg(unix)]
    fn call_socket(&self, path: &std::path::Path, method: &str, params: Value) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        });
        let payload = serde_json::to_vec(&request)
            .map_err(|e| AgentOSError::json(&e.to_string()))?;

        let mut last = String::new();
        for attempt in 0..=SVC_RETRIES {
            match Self::roundtrip(path, &payload, self.timeout) {
                Ok(raw) => {
                    let value: Value = serde_json::from_slice(&raw)
                        .map_err(|e| AgentOSError::json(&e.to_string()))?;
                    return extract_result(value);
                }
                Err(e) => {
                    last = e;
                    if attempt < SVC_RETRIES {
                        std::thread::sleep(Duration::from_millis(
                            SVC_BACKOFF_MS * (attempt as u64 + 1),
                        ));
                    }
                }
            }
        }
        Err(AgentOSError::network(&format!(
            "svc_call {method} on {} failed after retries: {last}",
            path.display()
        )))
    }

    #[cfg(unix)]
    fn roundtrip(
        path: &std::path::Path,
        payload: &[u8],
        timeout: Duration,
    ) -> std::result::Result<Vec<u8>, String> {
        let mut stream = UnixStream::connect(path).map_err(|e| e.to_string())?;
        stream.set_read_timeout(Some(timeout)).map_err(|e| e.to_string())?;
        stream.set_write_timeout(Some(timeout)).map_err(|e| e.to_string())?;
        stream.write_all(payload).map_err(|e| e.to_string())?;

        let mut buf = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            match stream.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    buf.extend_from_slice(&chunk[..n]);
                    // 守护按单帧应答：能整体解析即为完整响应。
                    if serde_json::from_slice::<Value>(&buf).is_ok() {
                        return Ok(buf);
                    }
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    if !buf.is_empty() && serde_json::from_slice::<Value>(&buf).is_ok() {
                        return Ok(buf);
                    }
                    return Err(format!("read timeout: {e}"));
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        if buf.is_empty() {
            Err("empty response".to_string())
        } else {
            Ok(buf)
        }
    }

    #[cfg(not(unix))]
    fn call_socket(&self, _path: &std::path::Path, _method: &str, _params: Value) -> Result<Value> {
        Err(AgentOSError::config(
            "svc_call requires AF_UNIX sockets (unsupported on this platform)",
        ))
    }
}

impl Default for SvcCall {
    fn default() -> Self {
        Self::from_env()
    }
}

fn runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("AIRY_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    let home = std::env::var("AIRY_HOME")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| {
            let user_home = std::env::var("HOME").unwrap_or_default();
            format!("{user_home}/.airymaxrt")
        });
    PathBuf::from(home).join("run")
}

fn extract_result(value: Value) -> Result<Value> {
    if let Some(result) = value.get("result") {
        return Ok(result.clone());
    }
    if let Some(error) = value.get("error") {
        return Err(AgentOSError::internal(&format!("jsonrpc error: {error}")));
    }
    Err(AgentOSError::json("svc_call response missing 'result'"))
}

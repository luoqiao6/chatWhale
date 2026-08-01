use crate::agent::mcp::types::{McpServerConfig, TransportKind};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

/// MCP stdio 传输：newline-delimited JSON-RPC 2.0。
pub struct McpTransport {
    child: Child,
    stdin: ChildStdin,
    reader: tokio::io::Lines<BufReader<ChildStdout>>,
    next_id: u64,
}

impl McpTransport {
    pub async fn spawn(config: &McpServerConfig) -> Result<Self> {
        if !matches!(config.transport, TransportKind::Stdio) {
            return Err(anyhow!("一期仅支持 stdio 传输"));
        }
        let mut cmd = tokio::process::Command::new(&config.command);
        cmd.args(&config.args)
            .envs(&config.env)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn().context("MCP 子进程启动失败")?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("无法获取 MCP stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("无法获取 MCP stdout"))?;
        let reader = BufReader::new(stdout).lines();
        Ok(Self {
            child,
            stdin,
            reader,
            next_id: 0,
        })
    }

    async fn send(&mut self, msg: Value) -> Result<()> {
        let line = serde_json::to_string(&msg)?;
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.write_all(b"\n").await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn recv_response(&mut self, id: u64, timeout: std::time::Duration) -> Result<Value> {
        tokio::time::timeout(timeout, async {
            loop {
                let line = self
                    .reader
                    .next_line()
                    .await
                    .context("MCP 流结束")?
                    .ok_or_else(|| anyhow!("MCP 流意外关闭"))?;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(v) = serde_json::from_str::<Value>(trimmed) else {
                    continue;
                };
                if v.get("id") == Some(&json!(id)) {
                    if let Some(err) = v.get("error") {
                        return Err(anyhow!("MCP 错误: {err}"));
                    }
                    return Ok(v.get("result").cloned().unwrap_or(Value::Null));
                }
            }
        })
        .await
        .map_err(|_| anyhow!("MCP 响应超时"))?
    }

    pub async fn initialize(&mut self) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": { "name": "chatwhale", "version": "1.0.0" }
            }
        }))
        .await?;
        self.recv_response(id, std::time::Duration::from_secs(10))
            .await
    }

    pub async fn notify_initialized(&mut self) -> Result<()> {
        self.send(json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }))
        .await
    }

    pub async fn list_tools(&mut self) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/list"
        }))
        .await?;
        self.recv_response(id, std::time::Duration::from_secs(10))
            .await
    }

    pub async fn call_tool(
        &mut self,
        name: &str,
        arguments: Value,
        timeout: std::time::Duration,
    ) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": arguments }
        }))
        .await?;
        self.recv_response(id, timeout).await
    }

    /// 发送 shutdown 通知并关闭子进程（防孤儿进程）。
    pub async fn shutdown(mut self) {
        let _ = self
            .send(json!({ "jsonrpc": "2.0", "method": "shutdown" }))
            .await;
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
    }
}

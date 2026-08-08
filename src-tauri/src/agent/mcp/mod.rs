pub mod transport;
pub mod types;

use crate::agent::types::{ToolDef, ToolResult};
use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use self::transport::McpTransport;
use self::types::{mcp_tool_name, McpServerConfig};

pub struct McpServerState {
    pub config: McpServerConfig,
    pub transport: Option<McpTransport>,
    pub tools: Vec<ToolDef>,
    pub lock: Arc<Mutex<()>>,
    pub healthy: bool,
    pub reconnect_attempted: bool,
}

pub struct McpManager {
    pub servers: HashMap<String, McpServerState>,
    pub name_mapping: HashMap<String, (String, String)>,
    pub failed: bool,
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            name_mapping: HashMap::new(),
            failed: false,
        }
    }

    pub async fn connect_all(&mut self, configs: Vec<McpServerConfig>) -> Result<()> {
        let mut first_err: Option<anyhow::Error> = None;
        for config in configs.into_iter().filter(|c| c.enabled) {
            match self.connect_one(config).await {
                Ok(()) => {}
                Err(e) => {
                    self.failed = true;
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }
        match first_err {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    async fn connect_one(&mut self, config: McpServerConfig) -> Result<()> {
        let mut transport = McpTransport::spawn(&config).await?;
        let _info = transport.initialize().await?;
        transport.notify_initialized().await?;
        let tools_raw = transport.list_tools().await?;

        let mut tools = Vec::new();
        let mut mapping: HashMap<String, (String, String)> = HashMap::new();
        if let Some(list) = tools_raw.get("tools").and_then(|t| t.as_array()) {
            for t in list {
                let Some(orig) = t.get("name").and_then(|v| v.as_str()) else {
                    continue;
                };
                let description = t
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let schema = t.get("inputSchema").cloned().unwrap_or(Value::Null);
                let mapped = mcp_tool_name(&config.id, orig);
                let mut final_name = mapped.clone();
                while self.name_mapping.contains_key(&final_name)
                    || mapping.contains_key(&final_name)
                {
                    let h = types::short_hash(orig);
                    let head: String = mapped.chars().take(64 - 9).collect();
                    final_name = format!("{head}_{h}");
                }
                mapping.insert(final_name.clone(), (config.id.clone(), orig.to_string()));
                tools.push(ToolDef::new(final_name, description, schema));
            }
        }

        let state = McpServerState {
            config,
            transport: Some(transport),
            tools,
            lock: Arc::new(Mutex::new(())),
            healthy: true,
            reconnect_attempted: false,
        };
        self.servers.insert(state.config.id.clone(), state);
        self.name_mapping.extend(mapping);
        Ok(())
    }

    pub fn all_tools(&self) -> Vec<ToolDef> {
        self.servers.values().flat_map(|s| s.tools.clone()).collect()
    }

    pub fn is_mcp_tool(&self, name: &str) -> bool {
        self.name_mapping.contains_key(name)
    }

    pub fn source_for(&self, name: &str) -> Option<String> {
        self.name_mapping
            .get(name)
            .map(|(sid, _)| format!("mcp: {sid}"))
    }

    pub async fn call_tool(&mut self, mapped_name: &str, arguments_json: &str) -> ToolResult {
        let Some((server_id, original)) = self.name_mapping.get(mapped_name).cloned() else {
            return ToolResult::error(format!("MCP 工具映射不存在: {mapped_name}"));
        };
        let args: Value = serde_json::from_str(arguments_json).unwrap_or(Value::Null);
        // 串行化同一 server 的调用（防 JSON-RPC 响应错位）
        let lock = match self.servers.get(&server_id) {
            Some(s) => s.lock.clone(),
            None => return ToolResult::error("MCP server 已断开"),
        };
        let _guard = lock.lock().await;
        let timeout = std::time::Duration::from_secs(
            self.servers
                .get(&server_id)
                .map(|s| s.config.timeout)
                .unwrap_or(30),
        );
        let result = {
            let state = self.servers.get_mut(&server_id).unwrap();
            let Some(transport) = state.transport.as_mut() else {
                return ToolResult::error("MCP server 连接不可用");
            };
            transport.call_tool(&original, args.clone(), timeout).await
        };
        match result {
            Ok(v) => {
                let content = extract_text(&v);
                ToolResult {
                    success: true,
                    content,
                    image_path: None,
                }
            }
            Err(e) => {
                self.mark_unhealthy(&server_id);
                if let Some(state) = self.servers.get(&server_id) {
                    if !state.reconnect_attempted {
                        let config = state.config.clone();
                        self.reconnect(&server_id, config).await;
                    }
                }
                ToolResult::error(format!("MCP 调用失败: {e}"))
            }
        }
    }

    fn mark_unhealthy(&mut self, server_id: &str) {
        if let Some(s) = self.servers.get_mut(server_id) {
            s.healthy = false;
        }
    }

    async fn reconnect(&mut self, server_id: &str, config: McpServerConfig) {
        match self.connect_one(config).await {
            Ok(()) => {}
            Err(_) => {
                self.failed = true;
                self.name_mapping.retain(|_, (sid, _)| sid != server_id);
                if let Some(s) = self.servers.get_mut(server_id) {
                    s.healthy = false;
                    s.tools.clear();
                }
            }
        }
        if let Some(s) = self.servers.get_mut(server_id) {
            s.reconnect_attempted = true;
        }
    }

    pub async fn shutdown_all(&mut self) {
        for state in self.servers.values_mut() {
            if let Some(t) = state.transport.take() {
                t.shutdown().await;
            }
        }
    }
}

fn extract_text(v: &Value) -> String {
    if let Some(content) = v.get("content") {
        let mut out = String::new();
        if let Some(arr) = content.as_array() {
            for item in arr {
                if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                    out.push_str(text);
                } else if let Some(json) = item.get("json") {
                    out.push_str(&serde_json::to_string(json).unwrap_or_default());
                }
            }
            if !out.is_empty() {
                return out;
            }
        }
        return serde_json::to_string(content).unwrap_or_default();
    }
    if v.get("isError").and_then(|e| e.as_bool()).unwrap_or(false) {
        return format!("Error: {}", serde_json::to_string(v).unwrap_or_default());
    }
    serde_json::to_string(v).unwrap_or_default()
}

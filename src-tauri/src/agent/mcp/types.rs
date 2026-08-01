use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportKind {
    #[default]
    Stdio,
    Sse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout: u64,
    #[serde(default)]
    pub transport: TransportKind,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_timeout() -> u64 {
    30
}
fn default_enabled() -> bool {
    true
}

/// LLM 侧工具名：mcp__<server_id>__<原始名>；非法字符替换为 _；超 64 字符或冲突追加 8 位短哈希。
pub fn mcp_tool_name(server_id: &str, original: &str) -> String {
    let sanitized: String = original
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = format!("mcp__{server_id}__{sanitized}");
    if base.len() <= 64 {
        base
    } else {
        let h = short_hash(original);
        let head: String = base.chars().take(64 - 9).collect();
        format!("{head}_{h}")
    }
}

/// FNV-1a 8 位十六进制短哈希，用于工具名冲突/超长处理。
pub fn short_hash(s: &str) -> String {
    let mut h: u32 = 0x811c9dc5;
    for b in s.as_bytes() {
        h ^= *b as u32;
        h = h.wrapping_mul(0x01000193);
    }
    format!("{:08x}", h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_and_namespaces_tool_names() {
        let n = mcp_tool_name("srv-a", "fetch data!");
        assert!(n.starts_with("mcp__srv-a__fetch_data_"));
        assert!(n.len() <= 64);
    }
}

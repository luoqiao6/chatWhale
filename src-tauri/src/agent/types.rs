use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// 一轮流式解析完成后的完整消息（含累积的 tool_calls）。
#[derive(Debug, Clone, Default)]
pub struct ChoiceMessage {
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

/// Rust 端权威的消息结构；序列化键与前端 Message 类型一致（snake_case）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl ChatMessage {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".into(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".into(),
            content: Some(content.into()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn assistant(content: Option<String>, reasoning_content: Option<String>) -> Self {
        Self {
            role: "assistant".into(),
            content,
            reasoning_content,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    pub fn assistant_with_tool_calls(message: &ChoiceMessage) -> Self {
        Self {
            role: "assistant".into(),
            content: message.content.clone(),
            reasoning_content: message.reasoning_content.clone(),
            tool_calls: if message.tool_calls.is_empty() {
                None
            } else {
                Some(message.tool_calls.clone())
            },
            tool_call_id: None,
            name: None,
        }
    }
    pub fn tool_result(tool_call_id: &str, result: &ToolResult) -> Self {
        Self {
            role: "tool".into(),
            content: Some(result.content.clone()),
            reasoning_content: None,
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
            name: None,
        }
    }
}

/// 把一轮流式结果里已生成的内容追加为 assistant 消息（仅当确有输出）。
/// 正常结束与异常结束（length/content_filter 等）分支共用，
/// 避免模型输出被截断时部分推理/内容丢失。
pub fn push_partial_assistant(messages: &mut Vec<ChatMessage>, message: &ChoiceMessage) {
    if message.content.is_some() || message.reasoning_content.is_some() {
        messages.push(ChatMessage::assistant(
            message.content.clone(),
            message.reasoning_content.clone(),
        ));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    #[serde(rename = "type")]
    pub def_type: String,
    pub function: ToolFunction,
}

impl ToolDef {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            def_type: "function".into(),
            function: ToolFunction {
                name: name.into(),
                description: description.into(),
                parameters,
                strict: None,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strict: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

/// 供并发累加的 usage 计数器（原子）。
#[derive(Debug, Default)]
pub struct UsageCounter {
    pub prompt_tokens: AtomicU64,
    pub completion_tokens: AtomicU64,
    pub total_tokens: AtomicU64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolResult {
    pub success: bool,
    pub content: String,
    #[serde(default)]
    pub image_path: Option<String>,
}

impl ToolResult {
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            content: format!("Error: {}", msg.into()),
            image_path: None,
        }
    }
}

impl UsageCounter {
    pub fn add(&self, u: &Usage) {
        self.prompt_tokens
            .fetch_add(u.prompt_tokens, Ordering::Relaxed);
        self.completion_tokens
            .fetch_add(u.completion_tokens, Ordering::Relaxed);
        self.total_tokens.fetch_add(u.total_tokens, Ordering::Relaxed);
    }
    /// 记录一次 LLM 流式调用的 usage。
    /// 同一流中 usage 可能被代理逐 chunk 重复携带，只以最后一次计入一次。
    pub fn record_stream_usage(&self, chunks: impl IntoIterator<Item = Usage>) {
        if let Some(last) = chunks.into_iter().last() {
            self.add(&last);
        }
    }
    pub fn snapshot(&self) -> Usage {
        Usage {
            prompt_tokens: self.prompt_tokens.load(Ordering::Relaxed),
            completion_tokens: self.completion_tokens.load(Ordering::Relaxed),
            total_tokens: self.total_tokens.load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApprovalPolicy {
    Always,
    Whitelist,
    Never,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhitelistEntry {
    pub prefix: String,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserContentPolicy {
    #[default]
    Strict,
    Normal,
    Trusted,
}

impl BrowserContentPolicy {
    pub fn as_str(&self) -> &'static str {
        match self {
            BrowserContentPolicy::Strict => "strict",
            BrowserContentPolicy::Normal => "normal",
            BrowserContentPolicy::Trusted => "trusted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserApproval {
    #[default]
    Navigation,
    Always,
}

pub fn parse_browser_policy(s: &str) -> BrowserContentPolicy {
    match s.trim() {
        "strict" => BrowserContentPolicy::Strict,
        "normal" => BrowserContentPolicy::Normal,
        "trusted" => BrowserContentPolicy::Trusted,
        _ => BrowserContentPolicy::Normal,
    }
}

pub fn parse_browser_approval(s: &str) -> BrowserApproval {
    match s.trim() {
        "navigation" => BrowserApproval::Navigation,
        "always" => BrowserApproval::Always,
        _ => BrowserApproval::Always,
    }
}

pub fn parse_domain_policy(s: &str) -> HashMap<String, BrowserContentPolicy> {
    serde_json::from_str::<HashMap<String, String>>(s)
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, parse_browser_policy(&v)))
        .collect()
}

/// 从 agent_settings 表解析出的运行时设置。
#[derive(Debug, Clone)]
pub struct AgentSettings {
    pub workspace_root: Option<PathBuf>,
    pub max_iterations: usize,
    pub skills_dir: Option<PathBuf>,
    pub command_approval: ApprovalPolicy,
    pub command_whitelist: Vec<WhitelistEntry>,
    pub llm_timeout: Duration,
    pub tool_timeout: Duration,
    pub command_timeout: Duration,
    pub approval_timeout: Duration,
    pub max_result_bytes: usize,
    pub sensitive_paths: Vec<String>,
    pub browser_enabled: bool,
    pub browser_path: Option<String>,
    pub browser_approval: BrowserApproval,
    pub browser_content_policy: BrowserContentPolicy,
    pub browser_domain_policy: HashMap<String, BrowserContentPolicy>,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            workspace_root: None,
            max_iterations: 10,
            skills_dir: None,
            command_approval: ApprovalPolicy::Always,
            command_whitelist: Vec::new(),
            llm_timeout: Duration::from_secs(60),
            tool_timeout: Duration::from_secs(30),
            command_timeout: Duration::from_secs(60),
            approval_timeout: Duration::from_secs(60),
            max_result_bytes: 204_800,
            sensitive_paths: Vec::new(),
            browser_enabled: true,
            browser_path: None,
            browser_approval: BrowserApproval::Always,
            browser_content_policy: BrowserContentPolicy::Normal,
            browser_domain_policy: HashMap::new(),
        }
    }
}

/// 预设 agent 设置 key 与默认值（与设计稿第 7 节一致）。
pub const AGENT_SETTING_KEYS: &[(&str, &str)] = &[
    ("agent.workspace_root", ""),
    ("agent.max_iterations", "10"),
    ("agent.skills_dir", ""),
    ("agent.command_approval", "always"),
    ("agent.command_whitelist", "[]"),
    ("agent.llm_timeout", "60"),
    ("agent.tool_timeout", "30"),
    ("agent.command_timeout", "60"),
    ("agent.approval_timeout", "60"),
    ("agent.max_result_bytes", "204800"),
    ("agent.sensitive_paths", "[]"),
    ("agent.browser_enabled", "true"),
    ("agent.browser_path", ""),
    ("agent.browser_approval", "always"),
    ("agent.browser_content_policy", "normal"),
    ("agent.browser_domain_policy", "{}"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentChatParams {
    pub messages: Vec<ChatMessage>,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub thinking: Option<serde_json::Value>,
    pub reasoning_effort: Option<String>,
}

pub fn parse_usize(s: &str, default: usize) -> usize {
    s.trim().parse().unwrap_or(default)
}

pub fn parse_duration_secs(s: &str, default: u64) -> Duration {
    Duration::from_secs(s.trim().parse().unwrap_or(default))
}

pub fn parse_policy(s: &str) -> ApprovalPolicy {
    match s.trim() {
        "whitelist" => ApprovalPolicy::Whitelist,
        "never" => ApprovalPolicy::Never,
        _ => ApprovalPolicy::Always,
    }
}

pub fn parse_whitelist(s: &str) -> Vec<WhitelistEntry> {
    serde_json::from_str(s).unwrap_or_default()
}

pub fn parse_string_list(s: &str) -> Vec<String> {
    serde_json::from_str(s).unwrap_or_default()
}

pub fn parse_bool(s: &str, default: bool) -> bool {
    match s.trim() {
        "true" | "1" => true,
        "false" | "0" => false,
        _ => default,
    }
}

pub fn load_agent_settings(map: &HashMap<String, String>) -> AgentSettings {
    let get = |k: &str, default: &str| {
        map.get(k)
            .map(|v| v.as_str())
            .unwrap_or(default)
            .to_string()
    };
    let workspace_root = get("agent.workspace_root", "").trim().to_string();
    let skills_dir = get("agent.skills_dir", "").trim().to_string();
    AgentSettings {
        workspace_root: if workspace_root.is_empty() {
            None
        } else {
            Some(PathBuf::from(workspace_root))
        },
        max_iterations: parse_usize(&get("agent.max_iterations", "10"), 10),
        skills_dir: if skills_dir.is_empty() {
            None
        } else {
            Some(PathBuf::from(skills_dir))
        },
        command_approval: parse_policy(&get("agent.command_approval", "always")),
        command_whitelist: parse_whitelist(&get("agent.command_whitelist", "[]")),
        llm_timeout: parse_duration_secs(&get("agent.llm_timeout", "60"), 60),
        tool_timeout: parse_duration_secs(&get("agent.tool_timeout", "30"), 30),
        command_timeout: parse_duration_secs(&get("agent.command_timeout", "60"), 60),
        approval_timeout: parse_duration_secs(&get("agent.approval_timeout", "60"), 60),
        max_result_bytes: parse_usize(&get("agent.max_result_bytes", "204800"), 204_800),
        sensitive_paths: parse_string_list(&get("agent.sensitive_paths", "[]")),
        browser_enabled: parse_bool(&get("agent.browser_enabled", "true"), true),
        browser_path: {
            let p = get("agent.browser_path", "").trim().to_string();
            if p.is_empty() { None } else { Some(p) }
        },
        browser_approval: parse_browser_approval(&get("agent.browser_approval", "always")),
        browser_content_policy: parse_browser_policy(&get("agent.browser_content_policy", "normal")),
        browser_domain_policy: parse_domain_policy(&get("agent.browser_domain_policy", "{}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settings_with_defaults() {
        let map = HashMap::new();
        let s = load_agent_settings(&map);
        assert_eq!(s.max_iterations, 10);
        assert_eq!(s.command_approval, ApprovalPolicy::Always);
        assert!(s.workspace_root.is_none());
    }

    #[test]
    fn parses_policy_variants() {
        assert_eq!(parse_policy("always"), ApprovalPolicy::Always);
        assert_eq!(parse_policy("whitelist"), ApprovalPolicy::Whitelist);
        assert_eq!(parse_policy("never"), ApprovalPolicy::Never);
        assert_eq!(parse_policy("garbage"), ApprovalPolicy::Always);
    }

    #[test]
    fn push_partial_assistant_preserves_partial_output() {
        let mut messages = vec![ChatMessage::user("hi")];
        let msg = ChoiceMessage {
            content: None,
            reasoning_content: Some("思考中".into()),
            tool_calls: vec![],
        };
        push_partial_assistant(&mut messages, &msg);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].reasoning_content.as_deref(), Some("思考中"));
        assert!(messages[1].content.is_none());
    }

    #[test]
    fn push_partial_assistant_skips_empty_message() {
        let mut messages = vec![ChatMessage::user("hi")];
        let msg = ChoiceMessage::default();
        push_partial_assistant(&mut messages, &msg);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn record_stream_usage_counts_last_chunk_once() {
        let counter = UsageCounter::default();
        // 同一流里代理可能逐 chunk 重复携带 usage，只应计入最后一次
        counter.record_stream_usage([
            Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            },
            Usage {
                prompt_tokens: 4,
                completion_tokens: 5,
                total_tokens: 9,
            },
        ]);
        let s = counter.snapshot();
        assert_eq!(s.prompt_tokens, 4);
        assert_eq!(s.completion_tokens, 5);
        assert_eq!(s.total_tokens, 9);
    }

    #[test]
    fn record_stream_usage_ignores_missing_usage() {
        let counter = UsageCounter::default();
        counter.record_stream_usage(None::<Usage>);
        assert_eq!(counter.snapshot().total_tokens, 0);
    }

    #[test]
    fn parses_browser_policy_variants() {
        assert_eq!(parse_browser_policy("strict"), BrowserContentPolicy::Strict);
        assert_eq!(parse_browser_policy("normal"), BrowserContentPolicy::Normal);
        assert_eq!(parse_browser_policy("trusted"), BrowserContentPolicy::Trusted);
        assert_eq!(parse_browser_policy("garbage"), BrowserContentPolicy::Normal);
        assert_eq!(BrowserContentPolicy::Trusted.as_str(), "trusted");
    }

    #[test]
    fn browser_policy_ordering() {
        assert!(BrowserContentPolicy::Strict < BrowserContentPolicy::Normal);
        assert!(BrowserContentPolicy::Normal < BrowserContentPolicy::Trusted);
    }

    #[test]
    fn parses_browser_approval_variants() {
        assert_eq!(parse_browser_approval("navigation"), BrowserApproval::Navigation);
        assert_eq!(parse_browser_approval("always"), BrowserApproval::Always);
        assert_eq!(parse_browser_approval("x"), BrowserApproval::Always);
    }

    #[test]
    fn parses_domain_policy_json() {
        let map = parse_domain_policy(r#"{"example.com":"trusted","*.foo.com":"normal"}"#);
        assert_eq!(map.get("example.com"), Some(&BrowserContentPolicy::Trusted));
        assert_eq!(map.get("*.foo.com"), Some(&BrowserContentPolicy::Normal));
        assert!(parse_domain_policy("not json").is_empty());
    }

    #[test]
    fn load_settings_includes_browser_defaults() {
        let map = HashMap::new();
        let s = load_agent_settings(&map);
        assert!(s.browser_enabled);
        assert!(s.browser_path.is_none());
        assert_eq!(s.browser_approval, BrowserApproval::Always);
        assert_eq!(s.browser_content_policy, BrowserContentPolicy::Normal);
        assert!(s.browser_domain_policy.is_empty());
    }
}

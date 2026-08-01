pub mod types;
pub mod llm;
pub mod approval;
pub mod tools;

use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::agent::types::UsageCounter;

pub const EVENT_CHUNK: &str = "agent-chunk";
pub const EVENT_REASONING: &str = "agent-reasoning";
pub const EVENT_TOOL_START: &str = "agent-tool-start";
pub const EVENT_TOOL_RESULT: &str = "agent-tool-result";
pub const EVENT_APPROVAL_REQUEST: &str = "agent-approval-request";
pub const EVENT_USAGE: &str = "agent-usage";
pub const EVENT_DONE: &str = "agent-done";
pub const EVENT_ERROR: &str = "agent-error";

pub fn emit_agent_event(
    app: &AppHandle,
    window_label: Option<&str>,
    event: &str,
    payload: impl Serialize + Clone,
) -> tauri::Result<()> {
    match window_label {
        Some(label) => app.emit_to(label, event, payload),
        None => app.emit(event, payload),
    }
}

/// 运行中的 Agent 状态（CancellationToken + usage 累计 + 发起窗口）。
pub struct AgentRuntime {
    pub cancellation: CancellationToken,
    pub usage: Arc<UsageCounter>,
    pub window_label: String,
}

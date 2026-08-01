pub mod types;
pub mod llm;
pub mod approval;
pub mod tools;
pub mod agent_config;
pub mod skills;

use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio_util::sync::CancellationToken;

use crate::agent::types::{ChatMessage, UsageCounter};

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

/// 只裁剪发送给模型的 messages，不动数据库与界面历史（12.7 v1 保底：保留最近 5 个工具轮）。
pub fn trim_messages_for_request(messages: &[ChatMessage]) -> Vec<ChatMessage> {
    const KEEP_ROUNDS: usize = 5;
    // 从后向前收集最近 KEEP_ROUNDS 个含 tool_calls 的轮次（assistant 索引）
    let mut round_stack: Vec<usize> = Vec::new();
    for (i, m) in messages.iter().enumerate().rev() {
        if m.tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
        {
            round_stack.push(i);
            if round_stack.len() >= KEEP_ROUNDS {
                break;
            }
        }
    }
    let keep_from = round_stack.last().copied().unwrap_or(0);
    let mut result = Vec::new();
    let mut i = 0;
    while i < messages.len() {
        let is_old_round = messages[i]
            .tool_calls
            .as_ref()
            .map(|t| !t.is_empty())
            .unwrap_or(false)
            && i < keep_from;
        if is_old_round {
            // 丢弃该 assistant 消息及其后相邻的 tool 结果（保持消息对完整）
            i += 1;
            while i < messages.len() && messages[i].role == "tool" {
                i += 1;
            }
        } else {
            result.push(messages[i].clone());
            i += 1;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::{ChatMessage, ToolCall, ToolResult};

    fn tool_msg(id: &str) -> ChatMessage {
        ChatMessage {
            role: "assistant".into(),
            content: None,
            reasoning_content: None,
            tool_calls: Some(vec![ToolCall {
                id: id.into(),
                call_type: "function".into(),
                function: crate::agent::types::FunctionCall {
                    name: "read_file".into(),
                    arguments: "{}".into(),
                },
            }]),
            tool_call_id: None,
            name: None,
        }
    }
    fn result_msg(id: &str) -> ChatMessage {
        ChatMessage::tool_result(
            id,
            &ToolResult {
                success: true,
                content: "ok".into(),
            },
        )
    }

    #[test]
    fn trims_to_keep_last_five_tool_rounds() {
        let mut msgs = vec![ChatMessage::user("hi")];
        for i in 0..7 {
            msgs.push(tool_msg(&format!("c{i}")));
            msgs.push(result_msg(&format!("c{i}")));
        }
        let trimmed = trim_messages_for_request(&msgs);
        let tool_msgs = trimmed.iter().filter(|m| m.tool_calls.is_some()).count();
        assert_eq!(tool_msgs, 5);
        assert_eq!(trimmed.first().unwrap().role, "user");
    }

    #[test]
    fn keeps_all_rounds_when_under_limit() {
        let msgs = vec![
            ChatMessage::user("hi"),
            tool_msg("c1"),
            result_msg("c1"),
        ];
        assert_eq!(trim_messages_for_request(&msgs).len(), 3);
    }
}

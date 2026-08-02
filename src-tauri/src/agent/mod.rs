pub mod types;
pub mod llm;
pub mod approval;
pub mod tools;
pub mod agent_config;
pub mod skills;
pub mod mcp;

use crate::agent::agent_config::AgentConfig;
use crate::agent::approval::{global_manager, ApprovalManager};
use crate::agent::llm::send_chat_stream;
use crate::agent::mcp::McpManager;
use crate::agent::skills::SkillManager;
use crate::agent::tools::{ToolContext, ToolRegistry};
use crate::agent::types::*;
use std::collections::HashMap;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;

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

/// Agent 入口：创建运行状态，执行回路，统一清理（单一出口，防 MCP 子进程泄漏）。
pub async fn run_agent(
    app: AppHandle,
    window_label: String,
    params: AgentChatParams,
    settings: AgentSettings,
    mcp_configs: Vec<crate::agent::mcp::types::McpServerConfig>,
) {
    let runtime = AgentRuntime {
        cancellation: CancellationToken::new(),
        usage: Arc::new(UsageCounter::default()),
        window_label: window_label.clone(),
    };
    run_agent_inner(&app, &window_label, &runtime, &params, &settings, mcp_configs).await;
    let state = app.state::<crate::AppState>();
    let mut guard = state.agent.lock().unwrap_or_else(|p| p.into_inner());
    *guard = None;
}

fn send_done(
    app: &AppHandle,
    window_label: &str,
    runtime: &AgentRuntime,
    messages: &[ChatMessage],
    reason: &str,
    finish_reason: Option<&str>,
    mcp_error: Option<&str>,
) -> tauri::Result<()> {
    emit_agent_event(
        app,
        Some(window_label),
        EVENT_DONE,
        serde_json::json!({
            "messages": messages,
            "reason": reason,
            "usage": runtime.usage.snapshot(),
            "finish_reason": finish_reason,
            "mcp_error": mcp_error,
        }),
    )
}

async fn run_agent_inner(
    app: &AppHandle,
    window_label: &str,
    runtime: &AgentRuntime,
    params: &AgentChatParams,
    settings: &AgentSettings,
    mcp_configs: Vec<crate::agent::mcp::types::McpServerConfig>,
) {
    let approval = global_manager();
    let mut mcp = McpManager::new();
    if let Err(e) = mcp.connect_all(mcp_configs).await {
        let _ = emit_agent_event(
            app,
            Some(window_label),
            EVENT_ERROR,
            serde_json::json!({ "message": format!("MCP 连接失败: {e}") }),
        );
    }

    // AGENT.md：首次加载需用户确认（防恶意仓库注入）
    let agent_config = AgentConfig::load(settings.workspace_root.as_deref());
    let mut system_prompt = agent_config.system_prompt_base();
    if let Some(md) = &agent_config.agent_md_content {
        let hash = content_hash(md);
        if approve_agent_md_if_needed(app, window_label, runtime, settings, approval, &hash).await
        {
            if let Some(frag) = agent_config.project_agent_md_fragment() {
                system_prompt.push_str(&frag);
            }
        }
    }

    // Skills：按最后一条用户消息匹配并注入
    let mut skill_manager =
        SkillManager::new(settings.skills_dir.clone(), settings.workspace_root.clone());
    let _ = skill_manager.load_all();
    let last_user = params
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .and_then(|m| m.content.clone())
        .unwrap_or_default();
    let matched = skill_manager.matching_skills(&last_user);
    system_prompt.push_str(&skill_manager.system_prompt_fragment(&matched));

    // 工具合并：内置 > 匹配技能 > MCP，上限 128
    let registry = ToolRegistry::with_builtins(settings);
    let mut tools: Vec<ToolDef> = registry.list_definitions();
    let mut tool_aliases: HashMap<String, String> = HashMap::new();
    for skill in &matched {
        for t in &skill.tools {
            let exists = registry.get(&t.uses).is_some() || mcp.is_mcp_tool(&t.uses);
            if !exists {
                continue;
            }
            tool_aliases.insert(t.name.clone(), t.uses.clone());
            let def = ToolDef::new(t.name.clone(), t.description.clone(), t.parameters.clone());
            if !tools.iter().any(|x| x.function.name == def.function.name) {
                tools.push(def);
            }
        }
    }
    for t in mcp.all_tools() {
        if !tools.iter().any(|x| x.function.name == t.function.name) {
            tools.push(t);
        }
    }
    tools.truncate(128);

    // 组装 messages：system 前置 + 裁剪后的历史
    let mut messages: Vec<ChatMessage> = Vec::new();
    messages.push(ChatMessage::system(system_prompt));
    messages.extend(trim_messages_for_request(&params.messages));

    for _ in 0..settings.max_iterations {
        tokio::select! {
            _ = runtime.cancellation.cancelled() => {
                mcp.shutdown_all().await;
                let _ = send_done(app, window_label, runtime, &messages, "cancelled", None, None);
                return;
            }
            result = send_chat_stream(app, Some(window_label), runtime, settings, &messages, &tools, params) => {
                let choice = match result {
                    Ok(c) => c,
                    Err(e) => {
                        let _ = emit_agent_event(app, Some(window_label), EVENT_ERROR, serde_json::json!({ "message": e.to_string() }));
                        mcp.shutdown_all().await;
                        let _ = send_done(app, window_label, runtime, &messages, "error", None, None);
                        return;
                    }
                };
                let final_reason = if mcp.failed { "mcp_error" } else { "stop" };
                match choice.finish_reason.as_deref() {
                    Some("stop") | None => {
                        push_partial_assistant(&mut messages, &choice.message);
                        mcp.shutdown_all().await;
                        let _ = send_done(
                            app,
                            window_label,
                            runtime,
                            &messages,
                            final_reason,
                            choice.finish_reason.as_deref(),
                            if mcp.failed {
                                Some("MCP server 连接失败，已剔除相关工具")
                            } else {
                                None
                            },
                        );
                        return;
                    }
                    Some("tool_calls") => {
                        messages.push(ChatMessage::assistant_with_tool_calls(&choice.message));
                        for tool_call in &choice.message.tool_calls {
                            if runtime.cancellation.is_cancelled() {
                                break;
                            }
                            let name = tool_aliases
                                .get(&tool_call.function.name)
                                .cloned()
                                .unwrap_or_else(|| tool_call.function.name.clone());
                            let source = mcp
                                .source_for(&name)
                                .unwrap_or_else(|| "builtin".to_string());
                            let _ = emit_agent_event(app, Some(window_label), EVENT_TOOL_START, serde_json::json!({
                                "id": tool_call.id,
                                "name": name,
                                "arguments": tool_call.function.arguments,
                                "source": source,
                            }));
                            let ctx = ToolContext {
                                app,
                                window_label: Some(window_label),
                                settings,
                                approval,
                                cancellation: runtime.cancellation.clone(),
                            };
                            let result = if mcp.is_mcp_tool(&name) {
                                mcp.call_tool(&name, &tool_call.function.arguments).await
                            } else {
                                let mut resolved_call = tool_call.clone();
                                if let Some(actual) = tool_aliases.get(&tool_call.function.name) {
                                    resolved_call.function.name = actual.clone();
                                }
                                registry.execute(&ctx, &resolved_call).await
                            };
                            // 统一出口脱敏 + 截断（内置与 MCP 结果一致）
                            let mut result = result;
                            result.content =
                                tools::finalize_result(result.content, settings.max_result_bytes);
                            let _ = emit_agent_event(app, Some(window_label), EVENT_TOOL_RESULT, serde_json::json!({
                                "id": tool_call.id,
                                "name": name,
                                "result": result.content,
                                "error": if result.success { None } else { Some(result.content.clone()) },
                            }));
                            messages.push(ChatMessage::tool_result(&tool_call.id, &result));
                        }
                    }
                    Some("length") | Some("content_filter") | Some("insufficient_system_resource") => {
                        // 输出被截断等异常结束：保留已流式生成的部分内容，并透传实际 finish_reason
                        push_partial_assistant(&mut messages, &choice.message);
                        mcp.shutdown_all().await;
                        let _ = send_done(
                            app,
                            window_label,
                            runtime,
                            &messages,
                            "finish_reason",
                            choice.finish_reason.as_deref(),
                            None,
                        );
                        return;
                    }
                    Some(_) => {
                        push_partial_assistant(&mut messages, &choice.message);
                        mcp.shutdown_all().await;
                        let _ = send_done(
                            app,
                            window_label,
                            runtime,
                            &messages,
                            final_reason,
                            choice.finish_reason.as_deref(),
                            None,
                        );
                        return;
                    }
                }
            }
        }
    }
    mcp.shutdown_all().await;
    let _ = send_done(app, window_label, runtime, &messages, "max_iterations", None, None);
}

async fn approve_agent_md_if_needed(
    app: &AppHandle,
    window_label: &str,
    runtime: &AgentRuntime,
    settings: &AgentSettings,
    approval: &ApprovalManager,
    hash: &str,
) -> bool {
    {
        let state = app.state::<crate::AppState>();
        let db = state.db.lock();
        if let Ok(db) = db {
            if let Ok(Some(v)) = db.get_agent_setting("agent.approved_agentmd") {
                if v.split(',').any(|h| h == hash) {
                    return true;
                }
            }
        }
    }
    match approval
        .request(
            app,
            Some(window_label),
            "AGENT.md",
            "首次加载工作目录的 AGENT.md 指令，是否启用？",
            "first_load",
            settings.approval_timeout,
            runtime.cancellation.clone(),
        )
        .await
    {
        approval::ApprovalOutcome::Granted => {
            let state = app.state::<crate::AppState>();
            let db = state.db.lock();
            if let Ok(db) = db {
                let cur = db
                    .get_agent_setting("agent.approved_agentmd")
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let mut list: Vec<String> = cur
                    .split(',')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
                    .collect();
                if !list.iter().any(|h| h == hash) {
                    list.push(hash.to_string());
                }
                let _ = db.set_agent_setting("agent.approved_agentmd", &list.join(","));
            }
            true
        }
        _ => false,
    }
}

/// FNV-1a 32 位哈希，用于 AGENT.md 内容指纹。
pub fn content_hash(s: &str) -> String {
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

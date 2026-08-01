use crate::agent::types::{AgentChatParams, AgentSettings, ChatMessage, ChoiceMessage, ToolCall, ToolDef, Usage};
use anyhow::{anyhow, Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tauri::AppHandle;

use super::{emit_agent_event, AgentRuntime};

#[derive(Debug, Clone, Default)]
pub struct ToolCallDelta {
    pub index: usize,
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ParsedDelta {
    pub reasoning: Option<String>,
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCallDelta>,
    pub finish_reason: Option<String>,
    pub usage: Option<Usage>,
}

#[derive(Debug)]
pub struct StreamChoice {
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct RawChunk {
    choices: Option<Vec<RawChoice>>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct RawChoice {
    delta: Option<RawDelta>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct RawDelta {
    reasoning_content: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<RawToolCallDelta>>,
}

#[derive(Deserialize)]
struct RawToolCallDelta {
    index: usize,
    id: Option<String>,
    function: Option<RawFunctionDelta>,
}

#[derive(Deserialize)]
struct RawFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

/// 解析单行 SSE（纯函数，便于测试）。`data: [DONE]`、空行、注释行返回 None。
pub fn parse_stream_chunk(line: &str) -> Option<ParsedDelta> {
    let line = line.trim();
    if line.is_empty()
        || line.starts_with(':')
        || !line.starts_with("data: ")
        || line[6..].trim() == "[DONE]"
    {
        return None;
    }
    let data = &line[6..];
    let chunk: RawChunk = serde_json::from_str(data).ok()?;
    let mut d = ParsedDelta::default();
    if let Some(usage) = chunk.usage {
        d.usage = Some(usage);
    }
    if let Some(choices) = chunk.choices {
        for choice in choices {
            if d.finish_reason.is_none() {
                d.finish_reason = choice.finish_reason;
            }
            if let Some(delta) = choice.delta {
                if d.reasoning.is_none() && delta.reasoning_content.is_some() {
                    d.reasoning = delta.reasoning_content;
                }
                if d.content.is_none() && delta.content.is_some() {
                    d.content = delta.content;
                }
                if let Some(tcs) = delta.tool_calls {
                    for tc in tcs {
                        d.tool_calls.push(ToolCallDelta {
                            index: tc.index,
                            id: tc.id,
                            name: tc.function.as_ref().and_then(|f| f.name.clone()),
                            arguments: tc.function.and_then(|f| f.arguments),
                        });
                    }
                }
            }
        }
    }
    Some(d)
}

pub fn thinking_enabled(params: &AgentChatParams) -> bool {
    params
        .thinking
        .as_ref()
        .and_then(|v| v.get("type"))
        .and_then(|v| v.as_str())
        .map(|t| t == "enabled")
        .unwrap_or(false)
}

/// Agent 循环内对 LLM 的调用：一律流式 SSE，解析增量并转发事件。
pub async fn send_chat_stream(
    app: &AppHandle,
    window_label: Option<&str>,
    runtime: &AgentRuntime,
    settings: &AgentSettings,
    messages: &[ChatMessage],
    tools: &[ToolDef],
    params: &AgentChatParams,
) -> Result<StreamChoice> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", params.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": params.model,
        "messages": messages,
        "stream": true,
        "stream_options": { "include_usage": true },
    });
    if let Some(t) = params.temperature {
        body["temperature"] = serde_json::json!(t);
    }
    if let Some(mt) = params.max_tokens {
        body["max_tokens"] = serde_json::json!(mt);
    }
    if thinking_enabled(params) {
        body["thinking"] = serde_json::json!({ "type": "enabled" });
        if let Some(e) = &params.reasoning_effort {
            body["reasoning_effort"] = serde_json::json!(e);
        }
    } else {
        body["thinking"] = serde_json::json!({ "type": "disabled" });
    }
    if !tools.is_empty() {
        body["tools"] = serde_json::json!(tools);
        body["tool_choice"] = serde_json::json!("auto");
    }

    let send = async {
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", params.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .with_context(|| format!("连接 LLM 失败: {url}"))?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "LLM API 错误 ({status}): {}",
                text.chars().take(500).collect::<String>()
            ));
        }

        let mut stream = response.bytes_stream();
        let mut buffer = String::new();
        let mut message = ChoiceMessage::default();
        let mut tool_acc: std::collections::BTreeMap<usize, ToolCall> = Default::default();
        let mut name_acc: std::collections::BTreeMap<usize, String> = Default::default();
        let mut args_acc: std::collections::BTreeMap<usize, String> = Default::default();
        let mut finish_reason: Option<String> = None;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("读取流失败")?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();
                let Some(delta) = parse_stream_chunk(&line) else {
                    continue;
                };
                if let Some(u) = &delta.usage {
                    runtime.usage.add(u);
                }
                if let Some(r) = delta.reasoning {
                    message
                        .reasoning_content
                        .get_or_insert_with(String::new)
                        .push_str(&r);
                    emit_agent_event(
                        app,
                        window_label,
                        super::EVENT_REASONING,
                        &serde_json::json!({ "content": r }),
                    )
                    .ok();
                }
                if let Some(c) = delta.content {
                    message.content.get_or_insert_with(String::new).push_str(&c);
                    emit_agent_event(
                        app,
                        window_label,
                        super::EVENT_CHUNK,
                        &serde_json::json!({ "content": c }),
                    )
                    .ok();
                }
                for tc in delta.tool_calls {
                    if let Some(id) = &tc.id {
                        let entry = tool_acc.entry(tc.index).or_insert_with(|| ToolCall {
                            id: id.clone(),
                            call_type: "function".into(),
                            function: crate::agent::types::FunctionCall {
                                name: String::new(),
                                arguments: String::new(),
                            },
                        });
                        entry.id = id.clone();
                    }
                    if let Some(n) = tc.name {
                        *name_acc.entry(tc.index).or_default() += &n;
                    }
                    if let Some(a) = tc.arguments {
                        *args_acc.entry(tc.index).or_default() += &a;
                    }
                }
                if let Some(fr) = delta.finish_reason {
                    if finish_reason.is_none() {
                        finish_reason = Some(fr);
                    }
                }
            }
        }
        for (idx, tc) in tool_acc.iter_mut() {
            tc.function.name = name_acc.remove(idx).unwrap_or_default();
            tc.function.arguments = args_acc.remove(idx).unwrap_or_default();
            message.tool_calls.push(tc.clone());
        }
        message.tool_calls.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(StreamChoice {
            message,
            finish_reason,
        })
    };

    let result = tokio::time::timeout(settings.llm_timeout, send)
        .await
        .map_err(|_| anyhow!("LLM 请求超时（{}s）", settings.llm_timeout.as_secs()))??;

    emit_agent_event(app, window_label, super::EVENT_USAGE, &runtime.usage.snapshot()).ok();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_content_and_reasoning_deltas() {
        let d = parse_stream_chunk(r#"data: {"choices":[{"delta":{"reasoning_content":"思考","content":"你好"}}]}"#).unwrap();
        assert_eq!(d.reasoning, Some("思考".to_string()));
        assert_eq!(d.content, Some("你好".to_string()));
    }

    #[test]
    fn parses_tool_call_fragments() {
        let d = parse_stream_chunk(r#"data: {"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"path\":"}}]}}]}"#).unwrap();
        let tc = d.tool_calls.first().unwrap();
        assert_eq!(tc.index, 0);
        assert_eq!(tc.id.as_deref(), Some("call_1"));
    }

    #[test]
    fn parses_usage_and_finish_reason() {
        let d = parse_stream_chunk(r#"data: {"choices":[{"finish_reason":"tool_calls"}],"usage":{"prompt_tokens":1,"completion_tokens":2,"total_tokens":3}}"#).unwrap();
        assert_eq!(d.finish_reason.as_deref(), Some("tool_calls"));
        assert_eq!(d.usage.as_ref().unwrap().total_tokens, 3);
    }

    #[test]
    fn skips_keepalive_and_empty_lines() {
        assert!(parse_stream_chunk(": keepalive").is_none());
        assert!(parse_stream_chunk("data: [DONE]").is_none());
        assert!(parse_stream_chunk("").is_none());
    }
}

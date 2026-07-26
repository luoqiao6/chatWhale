use anyhow::{Context, Result};
use tauri::Emitter;

use crate::ChatRequest;

pub async fn stream_chat(
    app_handle: tauri::AppHandle,
    request: ChatRequest,
) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", request.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": request.model,
        "messages": request.messages,
        "stream": true,
        "stream_options": {
            "include_usage": true
        }
    });

    if let Some(temp) = request.temperature {
        body["temperature"] = serde_json::json!(temp);
    }
    if let Some(mt) = request.max_tokens {
        body["max_tokens"] = serde_json::json!(mt);
    }
    if let Some(ref thinking) = request.thinking {
        body["thinking"] = thinking.clone();
    }
    if let Some(ref effort) = request.reasoning_effort {
        body["reasoning_effort"] = serde_json::json!(effort);
    }
    if let Some(ref tools) = request.tools {
        body["tools"] = tools.clone();
    }
    if let Some(ref tool_choice) = request.tool_choice {
        body["tool_choice"] = tool_choice.clone();
    }

    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", request.api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .with_context(|| format!("Failed to connect to {}", url))?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        app_handle
            .emit("chat-error", serde_json::json!({
                "status": status.as_u16(),
                "message": text,
            }))
            .ok();
        return Ok(());
    }

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read chunk")?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        while let Some(line_end) = buffer.find('\n') {
            let line = buffer[..line_end].trim().to_string();
            buffer = buffer[line_end + 1..].to_string();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            if line.starts_with("data: ") {
                let data = &line[6..];
                if data == "[DONE]" {
                    app_handle.emit("chat-done", ()).ok();
                } else {
                    app_handle
                        .emit("chat-chunk", data)
                        .ok();
                }
            }
        }
    }

    Ok(())
}

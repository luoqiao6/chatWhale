mod db;
mod sse;
pub mod agent;

use crate::agent::approval;
use crate::agent::types::{load_agent_settings, AgentChatParams, UsageCounter};
use crate::agent::AgentRuntime;
use db::Database;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

use crate::agent::mcp::types::McpServerConfig;
use crate::agent::tools::ToolRegistry;
use crate::agent::types::{AgentSettings, ToolDef};

pub struct AppState {
    pub db: Mutex<Database>,
    pub agent: Mutex<Option<AgentRuntime>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: String, // JSON string of messages array
    pub workspace_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub path: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
    pub conversation_count: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatRequest {
    pub messages: serde_json::Value,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
    pub temperature: Option<f64>,
    pub max_tokens: Option<i32>,
    pub thinking: Option<serde_json::Value>,
    pub reasoning_effort: Option<String>,
    pub tools: Option<serde_json::Value>,
    pub tool_choice: Option<serde_json::Value>,
    pub stream: Option<bool>,
}

#[tauri::command]
fn get_conversations(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<Conversation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_conversations(&workspace_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_workspaces(state: State<AppState>) -> Result<Vec<WorkspaceSummary>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_workspaces().map_err(|e| e.to_string())
}

#[tauri::command]
fn create_workspace(
    state: State<AppState>,
    name: String,
    path: String,
) -> Result<Workspace, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_workspace(&id, &name, &path).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_workspace(
    state: State<AppState>,
    id: String,
    name: Option<String>,
    path: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_workspace(&id, name.as_deref(), path.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_workspace_archived(
    state: State<AppState>,
    id: String,
    archived: bool,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.set_workspace_archived(&id, archived).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_workspace(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_workspace(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_conversation(state: State<AppState>, id: String) -> Result<Conversation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_conversation(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_conversation(
    state: State<AppState>,
    workspace_id: String,
    title: String,
    model: String,
) -> Result<Conversation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_conversation(&workspace_id, &title, &model)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn move_conversation(
    state: State<AppState>,
    id: String,
    workspace_id: String,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.move_conversation(&id, &workspace_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_conversation(
    state: State<AppState>,
    id: String,
    title: Option<String>,
    messages: Option<String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_conversation(&id, title.as_deref(), messages.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_conversation(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.delete_conversation(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_builtin_tools() -> Result<Vec<ToolDef>, String> {
    let registry = ToolRegistry::with_builtins(&AgentSettings::default());
    Ok(registry.list_definitions())
}

#[tauri::command]
fn list_mcp_servers(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<McpServerConfig>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_mcp_servers(&workspace_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn add_mcp_server(state: State<AppState>, server: McpServerConfig) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.add_mcp_server(&server).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_mcp_server(state: State<AppState>, id: String) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.remove_mcp_server(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn update_mcp_server(state: State<AppState>, server: McpServerConfig) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.update_mcp_server(&server).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_agent_settings(
    state: State<AppState>,
    workspace_id: String,
) -> Result<HashMap<String, String>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_all_agent_settings(&workspace_id).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_agent_settings(
    state: State<AppState>,
    workspace_id: String,
    settings: HashMap<String, String>,
) -> Result<(), String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    for (k, v) in settings {
        db.set_agent_setting(&workspace_id, &k, &v).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
async fn agent_chat(
    app: AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    params: AgentChatParams,
) -> Result<(), String> {
    {
        let mut guard = state.agent.lock().map_err(|e| e.to_string())?;
        if guard.is_some() {
            return Err("已有 Agent 正在运行，请等待其结束或先取消".into());
        }
        *guard = Some(AgentRuntime {
            cancellation: tokio_util::sync::CancellationToken::new(),
            usage: Arc::new(UsageCounter::default()),
            window_label: window.label().to_string(),
        });
    }
    let settings_map = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_all_agent_settings("default").map_err(|e| e.to_string())?
    };
    let mcp_configs = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.get_enabled_mcp_servers("default").map_err(|e| e.to_string())?
    };
    let settings = load_agent_settings(&settings_map);
    let window_label = window.label().to_string();
    tauri::async_runtime::spawn(async move {
        agent::run_agent(app, window_label, params, settings, mcp_configs).await;
    });
    Ok(())
}

#[tauri::command]
async fn agent_cancel(state: State<'_, AppState>) -> Result<(), String> {
    let guard = state.agent.lock().map_err(|e| e.to_string())?;
    if let Some(rt) = guard.as_ref() {
        rt.cancellation.cancel();
    }
    Ok(())
}

#[tauri::command]
async fn agent_approve(id: String, approved: bool) -> Result<(), String> {
    if approval::resolve_global(&id, approved) {
        Ok(())
    } else {
        Err(format!("审批请求 {id} 不存在或已超时"))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::new().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState {
            db: Mutex::new(db),
            agent: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            list_workspaces,
            create_workspace,
            update_workspace,
            set_workspace_archived,
            delete_workspace,
            get_conversations,
            get_conversation,
            move_conversation,
            create_conversation,
            update_conversation,
            delete_conversation,
            list_builtin_tools,
            list_mcp_servers,
            add_mcp_server,
            remove_mcp_server,
            update_mcp_server,
            get_agent_settings,
            set_agent_settings,
            agent_chat,
            agent_cancel,
            agent_approve,
        ])
        .run(tauri::generate_context!())
        .expect("error while running chatWhale");
}

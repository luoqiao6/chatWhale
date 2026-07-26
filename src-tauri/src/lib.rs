mod db;
mod sse;

use db::Database;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub db: Mutex<Database>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: String, // JSON string of messages array
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
fn get_conversations(state: State<AppState>) -> Result<Vec<Conversation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_conversations().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_conversation(state: State<AppState>, id: String) -> Result<Conversation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_conversation(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn create_conversation(
    state: State<AppState>,
    title: String,
    model: String,
) -> Result<Conversation, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.create_conversation(&title, &model)
        .map_err(|e| e.to_string())
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let db = Database::new().expect("Failed to initialize database");

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState { db: Mutex::new(db) })
        .invoke_handler(tauri::generate_handler![
            get_conversations,
            get_conversation,
            create_conversation,
            update_conversation,
            delete_conversation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running chatWhale");
}

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::Conversation;

use crate::agent::mcp::types::McpServerConfig;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new() -> Result<Self> {
        let db_path = Self::db_path()?;
        let conn = Connection::open(&db_path)
            .with_context(|| format!("Failed to open database at {:?}", db_path))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT 'deepseek-v4-pro',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                messages TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .context("Failed to create tables")?;
        Self::init_agent_tables(&conn)?;

        Ok(Self { conn })
    }

    fn init_agent_tables(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '[]',
                env TEXT NOT NULL DEFAULT '{}',
                cwd TEXT,
                timeout INTEGER NOT NULL DEFAULT 30,
                transport TEXT NOT NULL DEFAULT 'stdio',
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS agent_settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )
        .context("Failed to create agent tables")?;
        for (key, value) in crate::agent::types::AGENT_SETTING_KEYS {
            conn.execute(
                "INSERT OR IGNORE INTO agent_settings (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .context("Failed to seed agent settings")?;
        }
        Ok(())
    }

    fn db_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = PathBuf::from(home).join(".chatwhale");
        std::fs::create_dir_all(&path).ok();
        Ok(path.join("chatwhale.db"))
    }

    pub fn get_conversations(&self) -> Result<Vec<Conversation>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, model, created_at, updated_at, messages
                 FROM conversations ORDER BY updated_at DESC",
            )
            .context("Failed to prepare query")?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    messages: row.get(5)?,
                })
            })
            .context("Failed to query conversations")?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row?);
        }
        Ok(conversations)
    }

    pub fn get_conversation(&self, id: &str) -> Result<Conversation> {
        self.conn
            .query_row(
                "SELECT id, title, model, created_at, updated_at, messages
                 FROM conversations WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        model: row.get(2)?,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                        messages: row.get(5)?,
                    })
                },
            )
            .context("Conversation not found")
    }

    pub fn create_conversation(&self, title: &str, model: &str) -> Result<Conversation> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        self.conn
            .execute(
                "INSERT INTO conversations (id, title, model, created_at, updated_at, messages)
                 VALUES (?1, ?2, ?3, ?4, ?5, '[]')",
                params![id, title, model, now, now],
            )
            .context("Failed to create conversation")?;

        Ok(Conversation {
            id,
            title: title.to_string(),
            model: model.to_string(),
            created_at: now,
            updated_at: now,
            messages: "[]".to_string(),
        })
    }

    pub fn update_conversation(
        &self,
        id: &str,
        title: Option<&str>,
        messages: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();

        if let Some(t) = title {
            self.conn
                .execute(
                    "UPDATE conversations SET title = ?1, updated_at = ?2 WHERE id = ?3",
                    params![t, now, id],
                )
                .context("Failed to update title")?;
        }

        if let Some(m) = messages {
            self.conn
                .execute(
                    "UPDATE conversations SET messages = ?1, updated_at = ?2 WHERE id = ?3",
                    params![m, now, id],
                )
                .context("Failed to update messages")?;
        }

        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM conversations WHERE id = ?1", params![id])
            .context("Failed to delete conversation")?;
        Ok(())
    }

    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().context("open in-memory db")?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                model TEXT NOT NULL DEFAULT 'deepseek-v4-pro',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                messages TEXT NOT NULL DEFAULT '[]'
            );",
        )
        .context("Failed to create tables")?;
        Self::init_agent_tables(&conn)?;
        Ok(Self { conn })
    }

    pub fn get_all_agent_settings(&self) -> Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM agent_settings")
            .context("prepare agent settings")?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .context("query agent settings")?;
        let mut map = HashMap::new();
        for r in rows {
            let (k, v) = r?;
            map.insert(k, v);
        }
        Ok(map)
    }

    pub fn get_agent_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM agent_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .context("query agent setting")
    }

    pub fn set_agent_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO agent_settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .context("upsert agent setting")?;
        Ok(())
    }

    pub fn list_mcp_servers(&self) -> Result<Vec<McpServerConfig>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, command, args, env, cwd, timeout, transport, enabled
                 FROM mcp_servers ORDER BY created_at",
            )
            .context("prepare mcp servers")?;
        let rows = stmt
            .query_map([], row_to_server)
            .context("query mcp servers")?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(anyhow::Error::from)
    }

    pub fn get_enabled_mcp_servers(&self) -> Result<Vec<McpServerConfig>> {
        Ok(self
            .list_mcp_servers()?
            .into_iter()
            .filter(|s| s.enabled)
            .collect())
    }

    pub fn add_mcp_server(&self, cfg: &McpServerConfig) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn
            .execute(
                "INSERT INTO mcp_servers (id, name, command, args, env, cwd, timeout, transport, enabled, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    cfg.id,
                    cfg.name,
                    cfg.command,
                    serde_json::to_string(&cfg.args)?,
                    serde_json::to_string(&cfg.env)?,
                    cfg.cwd,
                    cfg.timeout as i64,
                    transport_str(cfg),
                    cfg.enabled as i64,
                    now,
                    now
                ],
            )
            .context("insert mcp server")?;
        Ok(())
    }

    pub fn update_mcp_server(&self, cfg: &McpServerConfig) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn
            .execute(
                "UPDATE mcp_servers SET name=?2, command=?3, args=?4, env=?5, cwd=?6, timeout=?7, transport=?8, enabled=?9, updated_at=?10 WHERE id=?1",
                params![
                    cfg.id,
                    cfg.name,
                    cfg.command,
                    serde_json::to_string(&cfg.args)?,
                    serde_json::to_string(&cfg.env)?,
                    cfg.cwd,
                    cfg.timeout as i64,
                    transport_str(cfg),
                    cfg.enabled as i64,
                    now
                ],
            )
            .context("update mcp server")?;
        Ok(())
    }

    pub fn remove_mcp_server(&self, id: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM mcp_servers WHERE id = ?1", params![id])
            .context("delete mcp server")?;
        Ok(())
    }
}

fn transport_str(cfg: &McpServerConfig) -> String {
    match cfg.transport {
        crate::agent::mcp::types::TransportKind::Sse => "sse".into(),
        crate::agent::mcp::types::TransportKind::Stdio => "stdio".into(),
    }
}

fn row_to_server(row: &rusqlite::Row) -> rusqlite::Result<McpServerConfig> {
    let transport: String = row.get(7)?;
    Ok(McpServerConfig {
        id: row.get(0)?,
        name: row.get(1)?,
        command: row.get(2)?,
        args: serde_json::from_str(&row.get::<_, String>(3)?).unwrap_or_default(),
        env: serde_json::from_str(&row.get::<_, String>(4)?).unwrap_or_default(),
        cwd: row.get(5)?,
        timeout: row.get::<_, i64>(6)? as u64,
        transport: if transport == "sse" {
            crate::agent::mcp::types::TransportKind::Sse
        } else {
            crate::agent::mcp::types::TransportKind::Stdio
        },
        enabled: row.get::<_, i64>(8)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::mcp::types::TransportKind;

    #[test]
    fn agent_settings_roundtrip() {
        let db = Database::in_memory().unwrap();
        db.set_agent_setting("agent.workspace_root", "/tmp")
            .unwrap();
        assert_eq!(
            db.get_agent_setting("agent.workspace_root").unwrap(),
            Some("/tmp".to_string())
        );
        let all = db.get_all_agent_settings().unwrap();
        assert!(all.contains_key("agent.workspace_root"));
    }

    #[test]
    fn mcp_servers_crud() {
        let db = Database::in_memory().unwrap();
        let cfg = McpServerConfig {
            id: "s1".into(),
            name: "S1".into(),
            command: "bash".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: TransportKind::Stdio,
            enabled: true,
        };
        db.add_mcp_server(&cfg).unwrap();
        assert_eq!(db.list_mcp_servers().unwrap().len(), 1);
        assert_eq!(db.get_enabled_mcp_servers().unwrap().len(), 1);
        db.remove_mcp_server("s1").unwrap();
        assert!(db.list_mcp_servers().unwrap().is_empty());
    }
}

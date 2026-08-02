use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use rusqlite::OptionalExtension;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::{Conversation, Workspace, WorkspaceSummary};

use crate::agent::mcp::types::McpServerConfig;
use crate::agent::types::{load_agent_settings, AgentSettings};

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
        Self::apply_migrations(&conn)?;

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
                workspace_id TEXT NOT NULL DEFAULT 'default',
                key TEXT NOT NULL,
                value TEXT NOT NULL,
                PRIMARY KEY (workspace_id, key)
            );",
        )
        .context("Failed to create agent tables")?;
        Ok(())
    }

    fn db_path() -> Result<PathBuf> {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let path = PathBuf::from(home).join(".chatwhale");
        std::fs::create_dir_all(&path).ok();
        Ok(path.join("chatwhale.db"))
    }

    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceSummary>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT w.id, w.name, w.path, w.archived, w.created_at, w.updated_at,
                        (SELECT COUNT(*) FROM conversations c WHERE c.workspace_id = w.id)
                 FROM workspaces w ORDER BY w.created_at",
            )
            .context("Failed to prepare workspaces query")?;
        let rows = stmt
            .query_map([], |row| {
                Ok(WorkspaceSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    archived: row.get::<_, i64>(3)? != 0,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    conversation_count: row.get(6)?,
                })
            })
            .context("Failed to query workspaces")?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(anyhow::Error::from)
    }

    pub fn get_workspace(&self, id: &str) -> Result<Option<Workspace>> {
        self.conn
            .query_row(
                "SELECT id, name, path, archived, created_at, updated_at
                 FROM workspaces WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Workspace {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        archived: row.get::<_, i64>(3)? != 0,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()
            .context("Failed to query workspace")
    }

    pub fn create_workspace(&self, id: &str, name: &str, path: &str) -> Result<Workspace> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn
            .execute(
                "INSERT INTO workspaces (id, name, path, archived, created_at, updated_at)
                 VALUES (?1, ?2, ?3, 0, ?4, ?4)",
                params![id, name, path, now],
            )
            .context("Failed to create workspace")?;
        // 新空间按默认键 seed，保证各空间拥有独立完整的设置集合
        for (key, value) in crate::agent::types::AGENT_SETTING_KEYS {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO agent_settings (workspace_id, key, value)
                     VALUES (?1, ?2, ?3)",
                    params![id, key, value],
                )
                .context("Failed to seed agent settings")?;
        }
        Ok(Workspace {
            id: id.to_string(),
            name: name.to_string(),
            path: path.to_string(),
            archived: false,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn update_workspace(
        &self,
        id: &str,
        name: Option<&str>,
        path: Option<&str>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        if let Some(n) = name {
            self.conn
                .execute(
                    "UPDATE workspaces SET name = ?1, updated_at = ?2 WHERE id = ?3",
                    params![n, now, id],
                )
                .context("Failed to update workspace name")?;
        }
        if let Some(p) = path {
            self.conn
                .execute(
                    "UPDATE workspaces SET path = ?1, updated_at = ?2 WHERE id = ?3",
                    params![p, now, id],
                )
                .context("Failed to update workspace path")?;
        }
        Ok(())
    }

    pub fn set_workspace_archived(&self, id: &str, archived: bool) -> Result<()> {
        if id == "default" && archived {
            anyhow::bail!("默认工作空间不可归档");
        }
        self.conn
            .execute(
                "UPDATE workspaces SET archived = ?1, updated_at = ?2 WHERE id = ?3",
                params![archived as i64, chrono::Utc::now().timestamp_millis(), id],
            )
            .context("Failed to update workspace archived")?;
        Ok(())
    }

    pub fn delete_workspace(&self, id: &str) -> Result<()> {
        if id == "default" {
            anyhow::bail!("默认工作空间不可删除");
        }
        self.conn
            .execute_batch("BEGIN IMMEDIATE")
            .context("Failed to begin workspace delete")?;
        let result = (|| -> Result<()> {
            self.conn
                .execute("DELETE FROM conversations WHERE workspace_id = ?1", params![id])
                .context("Failed to delete workspace conversations")?;
            self.conn
                .execute("DELETE FROM agent_settings WHERE workspace_id = ?1", params![id])
                .context("Failed to delete workspace settings")?;
            self.conn
                .execute("DELETE FROM mcp_servers WHERE workspace_id = ?1", params![id])
                .context("Failed to delete workspace mcp servers")?;
            self.conn
                .execute("DELETE FROM workspaces WHERE id = ?1", params![id])
                .context("Failed to delete workspace")?;
            Ok(())
        })();
        if result.is_err() {
            let _ = self.conn.execute_batch("ROLLBACK");
            return result;
        }
        self.conn
            .execute_batch("COMMIT")
            .context("Failed to commit workspace delete")
    }

    pub fn copy_workspace_settings(&self, from_id: &str, to_id: &str) -> Result<()> {
        if from_id == to_id {
            return Ok(());
        }
        let Some(source) = self.get_all_agent_settings(from_id).ok().filter(|m| !m.is_empty()) else {
            // 来源不存在或为空：仅保留目标空间已 seed 的默认键
            return Ok(());
        };
        for (key, value) in source {
            self.set_agent_setting(to_id, &key, &value)?;
        }
        let servers = self.list_mcp_servers(from_id)?;
        for mut s in servers {
            s.id = uuid::Uuid::new_v4().to_string();
            s.workspace_id = to_id.to_string();
            self.add_mcp_server(&s)?;
        }
        Ok(())
    }

    pub fn build_agent_settings(
        &self,
        workspace_id: &str,
    ) -> Result<(AgentSettings, Vec<McpServerConfig>)> {
        let ws = self
            .get_workspace(workspace_id)?
            .ok_or_else(|| anyhow::anyhow!("工作空间不存在"))?;
        if ws.archived {
            anyhow::bail!("该工作空间已归档，请先恢复后再继续对话");
        }
        let mut map = self.get_all_agent_settings(workspace_id)?;
        // workspaces.path 是工作目录唯一权威来源，覆盖（退役）设置键
        if ws.path.is_empty() {
            map.remove("agent.workspace_root");
        } else {
            map.insert("agent.workspace_root".to_string(), ws.path.clone());
        }
        let settings = load_agent_settings(&map);
        let mcp = self.get_enabled_mcp_servers(workspace_id)?;
        Ok((settings, mcp))
    }

    pub fn get_conversations(&self, workspace_id: &str) -> Result<Vec<Conversation>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, title, model, created_at, updated_at, messages, workspace_id
                 FROM conversations WHERE workspace_id = ?1 ORDER BY updated_at DESC",
            )
            .context("Failed to prepare query")?;

        let rows = stmt
            .query_map(params![workspace_id], |row| {
                Ok(Conversation {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    model: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    messages: row.get(5)?,
                    workspace_id: row.get(6)?,
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
                "SELECT id, title, model, created_at, updated_at, messages, workspace_id
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
                        workspace_id: row.get(6)?,
                    })
                },
            )
            .context("Conversation not found")
    }

    pub fn create_conversation(
        &self,
        workspace_id: &str,
        title: &str,
        model: &str,
    ) -> Result<Conversation> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();

        self.conn
            .execute(
                "INSERT INTO conversations (id, title, model, created_at, updated_at, messages, workspace_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, '[]', ?6)",
                params![id, title, model, now, now, workspace_id],
            )
            .context("Failed to create conversation")?;

        Ok(Conversation {
            id,
            title: title.to_string(),
            model: model.to_string(),
            created_at: now,
            updated_at: now,
            messages: "[]".to_string(),
            workspace_id: workspace_id.to_string(),
        })
    }

    pub fn move_conversation(&self, id: &str, workspace_id: &str) -> Result<()> {
        self.conn
            .execute(
                "UPDATE conversations SET workspace_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![workspace_id, chrono::Utc::now().timestamp_millis(), id],
            )
            .context("Failed to move conversation")?;
        Ok(())
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
        Self::apply_migrations(&conn)?;
        Ok(Self { conn })
    }

    pub fn get_all_agent_settings(&self, workspace_id: &str) -> Result<HashMap<String, String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value FROM agent_settings WHERE workspace_id = ?1")
            .context("prepare agent settings")?;
        let rows = stmt
            .query_map(params![workspace_id], |row| {
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

    pub fn get_agent_setting(
        &self,
        workspace_id: &str,
        key: &str,
    ) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM agent_settings WHERE workspace_id = ?1 AND key = ?2",
                params![workspace_id, key],
                |row| row.get(0),
            )
            .optional()
            .context("query agent setting")
    }

    pub fn set_agent_setting(&self, workspace_id: &str, key: &str, value: &str) -> Result<()> {
        self.conn
            .execute(
                "INSERT INTO agent_settings (workspace_id, key, value) VALUES (?1, ?2, ?3)
                 ON CONFLICT(workspace_id, key) DO UPDATE SET value = excluded.value",
                params![workspace_id, key, value],
            )
            .context("upsert agent setting")?;
        Ok(())
    }

    pub fn list_mcp_servers(&self, workspace_id: &str) -> Result<Vec<McpServerConfig>> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, name, command, args, env, cwd, timeout, transport, enabled, workspace_id
                 FROM mcp_servers WHERE workspace_id = ?1 ORDER BY created_at",
            )
            .context("prepare mcp servers")?;
        let rows = stmt
            .query_map(params![workspace_id], row_to_server)
            .context("query mcp servers")?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(anyhow::Error::from)
    }

    pub fn get_enabled_mcp_servers(&self, workspace_id: &str) -> Result<Vec<McpServerConfig>> {
        Ok(self
            .list_mcp_servers(workspace_id)?
            .into_iter()
            .filter(|s| s.enabled)
            .collect())
    }

    pub fn add_mcp_server(&self, cfg: &McpServerConfig) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        self.conn
            .execute(
                "INSERT INTO mcp_servers (id, name, command, args, env, cwd, timeout, transport, enabled, created_at, updated_at, workspace_id)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
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
                    now,
                    cfg.workspace_id
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

fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let cols = stmt.query_map([], |r| r.get::<_, String>(1))?;
    for c in cols {
        if c? == column {
            return Ok(true);
        }
    }
    Ok(false)
}

impl Database {
    /// 幂等迁移：workspaces 建表、加列、agent_settings 换表、seed、默认空间创建。
    pub(crate) fn apply_migrations(conn: &Connection) -> Result<()> {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workspaces (
                id         TEXT PRIMARY KEY,
                name       TEXT NOT NULL,
                path       TEXT NOT NULL DEFAULT '',
                archived   INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );",
        )
        .context("Failed to create workspaces table")?;

        if !column_exists(conn, "conversations", "workspace_id")? {
            conn.execute_batch(
                "ALTER TABLE conversations ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
                 CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id);",
            )
            .context("Failed to migrate conversations.workspace_id")?;
        }
        if !column_exists(conn, "mcp_servers", "workspace_id")? {
            conn.execute_batch(
                "ALTER TABLE mcp_servers ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
                 CREATE INDEX IF NOT EXISTS idx_mcp_servers_workspace ON mcp_servers(workspace_id);",
            )
            .context("Failed to migrate mcp_servers.workspace_id")?;
        }
        if !column_exists(conn, "agent_settings", "workspace_id")? {
            conn.execute_batch(
                "ALTER TABLE agent_settings RENAME TO agent_settings_legacy;
                 CREATE TABLE agent_settings (
                    workspace_id TEXT NOT NULL DEFAULT 'default',
                    key          TEXT NOT NULL,
                    value        TEXT NOT NULL,
                    PRIMARY KEY (workspace_id, key)
                 );
                 INSERT INTO agent_settings (workspace_id, key, value)
                     SELECT 'default', key, value FROM agent_settings_legacy;
                 DROP TABLE agent_settings_legacy;",
            )
            .context("Failed to migrate agent_settings")?;
        }

        for (key, value) in crate::agent::types::AGENT_SETTING_KEYS {
            conn.execute(
                "INSERT OR IGNORE INTO agent_settings (workspace_id, key, value)
                 VALUES ('default', ?1, ?2)",
                params![key, value],
            )
            .context("Failed to seed agent settings")?;
        }

        let has_default: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = 'default')",
                [],
                |r| r.get::<_, i64>(0),
            )?
            != 0;
        if !has_default {
            let path: String = conn
                .query_row(
                    "SELECT value FROM agent_settings
                     WHERE workspace_id = 'default' AND key = 'agent.workspace_root'",
                    [],
                    |r| r.get(0),
                )
                .optional()?
                .unwrap_or_default();
            let now = chrono::Utc::now().timestamp_millis();
            conn.execute(
                "INSERT INTO workspaces (id, name, path, archived, created_at, updated_at)
                 VALUES ('default', '默认工作空间', ?1, 0, ?2, ?2)",
                params![path, now],
            )
            .context("Failed to create default workspace")?;
            // workspace_root 退役：路径已复制到 workspaces.path，移除设置键避免双源
            conn.execute(
                "DELETE FROM agent_settings
                 WHERE workspace_id = 'default' AND key = 'agent.workspace_root'",
                [],
            )
            .ok();
        }
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
        workspace_id: row.get(9)?,
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
    use rusqlite::Connection;

    #[test]
    fn migrate_creates_workspaces_table_with_default() {
        let db = Database::in_memory().unwrap();
        let count: i64 = db
            .conn
            .query_row(
                "SELECT COUNT(*) FROM workspaces WHERE id = 'default'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let (name, path): (String, String) = db
            .conn
            .query_row(
                "SELECT name, path FROM workspaces WHERE id = 'default'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "默认工作空间");
        assert_eq!(path, "");
    }

    #[test]
    fn migrate_legacy_db_preserves_workspace_root_into_default_path() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations (
                id TEXT PRIMARY KEY, title TEXT NOT NULL, model TEXT NOT NULL,
                created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL,
                messages TEXT NOT NULL DEFAULT '[]'
            );
            CREATE TABLE mcp_servers (
                id TEXT PRIMARY KEY, name TEXT NOT NULL, command TEXT NOT NULL,
                args TEXT NOT NULL DEFAULT '[]', env TEXT NOT NULL DEFAULT '{}', cwd TEXT,
                timeout INTEGER NOT NULL DEFAULT 30, transport TEXT NOT NULL DEFAULT 'stdio',
                enabled INTEGER NOT NULL DEFAULT 1, created_at INTEGER NOT NULL, updated_at INTEGER NOT NULL
            );
            CREATE TABLE agent_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            INSERT INTO agent_settings (key, value) VALUES ('agent.workspace_root', '/tmp/proj');
            INSERT INTO conversations (id, title, model, created_at, updated_at, messages)
                VALUES ('c1', '旧会话', 'm', 1, 1, '[]');",
        )
        .unwrap();
        Database::apply_migrations(&conn).unwrap();

        let path: String = conn
            .query_row("SELECT path FROM workspaces WHERE id='default'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(path, "/tmp/proj");

        let ws_id: String = conn
            .query_row(
                "SELECT workspace_id FROM conversations WHERE id='c1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ws_id, "default");

        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM agent_settings WHERE key='agent.workspace_root'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn migrate_is_idempotent() {
        let db = Database::in_memory().unwrap();
        Database::apply_migrations(&db.conn).unwrap();
        Database::apply_migrations(&db.conn).unwrap();
        let count: i64 = db
            .conn
            .query_row("SELECT COUNT(*) FROM workspaces", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn workspace_crud_and_default_protection() {
        let db = Database::in_memory().unwrap();

        let ws = db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        assert_eq!(ws.name, "项目A");
        assert!(!ws.archived);

        let all = db.list_workspaces().unwrap();
        assert_eq!(all.len(), 2); // default + w1
        let w1 = all.iter().find(|w| w.id == "w1").unwrap();
        assert_eq!(w1.conversation_count, 0);

        db.update_workspace("w1", Some("项目A改"), None).unwrap();
        assert_eq!(db.get_workspace("w1").unwrap().unwrap().name, "项目A改");

        db.set_workspace_archived("w1", true).unwrap();
        assert!(db.get_workspace("w1").unwrap().unwrap().archived);

        // 默认空间不可归档/删除
        assert!(db.set_workspace_archived("default", true).is_err());
        assert!(db.delete_workspace("default").is_err());
    }

    #[test]
    fn settings_and_mcp_are_scoped_by_workspace() {
        let db = Database::in_memory().unwrap();
        db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        db.create_workspace("w2", "项目B", "/tmp/b").unwrap();

        db.set_agent_setting("w1", "agent.max_iterations", "3").unwrap();
        db.set_agent_setting("w2", "agent.max_iterations", "7").unwrap();
        assert_eq!(
            db.get_all_agent_settings("w1").unwrap().get("agent.max_iterations").map(|s| s.as_str()),
            Some("3")
        );
        assert_eq!(
            db.get_all_agent_settings("w2").unwrap().get("agent.max_iterations").map(|s| s.as_str()),
            Some("7")
        );
        // 各空间至少有自己的 seed 键，但互不包含对方的值
        assert!(db.get_all_agent_settings("w1").unwrap().len() > 1);

        let server = |id: &str, ws: &str| McpServerConfig {
            id: id.into(),
            workspace_id: ws.into(),
            name: "fs".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: crate::agent::mcp::types::TransportKind::Stdio,
            enabled: true,
        };
        db.add_mcp_server(&server("s1", "w1")).unwrap();
        db.add_mcp_server(&server("s2", "w2")).unwrap();
        assert_eq!(db.list_mcp_servers("w1").unwrap().len(), 1);
        assert_eq!(db.list_mcp_servers("w2").unwrap().len(), 1);
        assert_eq!(db.get_enabled_mcp_servers("w1").unwrap()[0].id, "s1");
    }

    #[test]
    fn conversations_are_scoped_and_movable() {
        let db = Database::in_memory().unwrap();
        db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        db.create_workspace("w2", "项目B", "/tmp/b").unwrap();
        let c = db.create_conversation("w1", "会话A", "m").unwrap();
        assert_eq!(db.get_conversations("w1").unwrap().len(), 1);
        assert_eq!(db.get_conversations("w2").unwrap().len(), 0);

        db.move_conversation(&c.id, "w2").unwrap();
        assert_eq!(db.get_conversations("w1").unwrap().len(), 0);
        assert_eq!(db.get_conversations("w2").unwrap().len(), 1);
    }

    #[test]
    fn delete_workspace_cascades() {
        let db = Database::in_memory().unwrap();
        db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        db.create_conversation("w1", "会话", "m").unwrap();
        db.set_agent_setting("w1", "agent.max_iterations", "3").unwrap();
        let server = McpServerConfig {
            id: "s1".into(),
            workspace_id: "w1".into(),
            name: "fs".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: crate::agent::mcp::types::TransportKind::Stdio,
            enabled: true,
        };
        db.add_mcp_server(&server).unwrap();

        db.delete_workspace("w1").unwrap();
        assert!(db.get_workspace("w1").unwrap().is_none());
        assert_eq!(db.get_conversations("w1").unwrap().len(), 0);
        assert_eq!(db.get_all_agent_settings("w1").unwrap().len(), 0);
        assert_eq!(db.list_mcp_servers("w1").unwrap().len(), 0);
    }

    #[test]
    fn copy_workspace_settings_copies_values_with_new_mcp_ids() {
        let db = Database::in_memory().unwrap();
        db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        db.set_agent_setting("w1", "agent.max_iterations", "3").unwrap();
        db.add_mcp_server(&McpServerConfig {
            id: "s1".into(),
            workspace_id: "w1".into(),
            name: "fs".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: crate::agent::mcp::types::TransportKind::Stdio,
            enabled: true,
        })
        .unwrap();

        db.create_workspace("w2", "项目B", "/tmp/b").unwrap();
        db.copy_workspace_settings("w1", "w2").unwrap();

        assert_eq!(
            db.get_all_agent_settings("w2").unwrap().get("agent.max_iterations").map(|s| s.as_str()),
            Some("3")
        );
        let servers = db.list_mcp_servers("w2").unwrap();
        assert_eq!(servers.len(), 1);
        assert_ne!(servers[0].id, "s1");
        assert_eq!(servers[0].name, "fs");

        // 不存在的来源：不报错、目标空间无复制内容（仅 seed 默认键）
        db.copy_workspace_settings("nope", "w2").unwrap();
    }

    #[test]
    fn build_agent_settings_injects_workspace_path_and_filters_mcp() {
        let db = Database::in_memory().unwrap();
        db.update_workspace("default", None, Some("/data/proj")).unwrap();
        db.set_agent_setting("default", "agent.max_iterations", "5").unwrap();
        db.add_mcp_server(&McpServerConfig {
            id: "s1".into(),
            workspace_id: "default".into(),
            name: "fs".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: crate::agent::mcp::types::TransportKind::Stdio,
            enabled: true,
        })
        .unwrap();
        db.add_mcp_server(&McpServerConfig {
            id: "s2".into(),
            workspace_id: "default".into(),
            name: "off".into(),
            command: "npx".into(),
            args: vec![],
            env: Default::default(),
            cwd: None,
            timeout: 30,
            transport: crate::agent::mcp::types::TransportKind::Stdio,
            enabled: false,
        })
        .unwrap();

        let (settings, mcp) = db.build_agent_settings("default").unwrap();
        assert_eq!(settings.workspace_root.as_deref(), Some(std::path::Path::new("/data/proj")));
        assert_eq!(settings.max_iterations, 5);
        assert_eq!(mcp.len(), 1);
        assert_eq!(mcp[0].id, "s1");
    }

    #[test]
    fn build_agent_settings_rejects_archived_workspace() {
        let db = Database::in_memory().unwrap();
        db.create_workspace("w1", "项目A", "/tmp/a").unwrap();
        db.set_workspace_archived("w1", true).unwrap();
        assert!(db.build_agent_settings("w1").is_err());
    }

    #[test]
    fn agent_settings_roundtrip() {
        let db = Database::in_memory().unwrap();
        db.set_agent_setting("default", "agent.max_iterations", "7")
            .unwrap();
        assert_eq!(
            db.get_agent_setting("default", "agent.max_iterations").unwrap(),
            Some("7".to_string())
        );
        let all = db.get_all_agent_settings("default").unwrap();
        assert!(all.contains_key("agent.max_iterations"));
    }

    #[test]
    fn mcp_servers_crud() {
        let db = Database::in_memory().unwrap();
        let cfg = McpServerConfig {
            id: "s1".into(),
            workspace_id: "default".into(),
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
        assert_eq!(db.list_mcp_servers("default").unwrap().len(), 1);
        assert_eq!(db.get_enabled_mcp_servers("default").unwrap().len(), 1);
        db.remove_mcp_server("s1").unwrap();
        assert!(db.list_mcp_servers("default").unwrap().is_empty());
    }
}

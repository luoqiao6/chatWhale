use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::path::PathBuf;

use crate::Conversation;

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

        Ok(Self { conn })
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
}

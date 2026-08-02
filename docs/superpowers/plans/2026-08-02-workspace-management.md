# 工作空间管理 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 chatWhale 的全局"工作目录"升级为完整工作区（目录 + 独立 Agent 设置/MCP + 归属会话历史），提供侧边栏一键切换与空间管理界面。

**Architecture:** SQLite 新增 `workspaces` 表，`conversations`/`mcp_servers` 加 `workspace_id` 列、`agent_settings` 改为 `(workspace_id, key)` 复合主键，内置幂等迁移（首次启动创建"默认工作空间"并归位旧数据）；前端新增 `useWorkspaces` 单例与 `WorkspaceSwitcher`/`WorkspaceManager` 组件，会话数据统一走 Rust 命令（浏览器模式降级 localStorage）。

**Tech Stack:** Tauri v2 + rusqlite（Rust 后端）、Vue 3.5 + TypeScript 5.7 + Vitest 4（前端）。

## Global Constraints

- 默认工作空间 id 固定为 `"default"`，**不可归档、不可删除**，可重命名。
- API Key 仅存于 localStorage（`chatwhale-api-key`），不落库、不进日志、不进源码。
- 每个任务结束时必须提交；代码提交前运行 `npm test`、`npm run typecheck`、`npm run build`；Rust 改动运行 `cd src-tauri && cargo test`，全部通过才提交。
- 浏览器模式（`npm run dev`，无 Tauri）必须降级可用：空间与会话存 localStorage，Agent 设置/MCP 命令失败仅提示，不崩溃。
- 所有 UI 文案使用简体中文；`v-html` 渲染模型内容的净化边界不改变。
- Rust 结构体字段与命令参数为 snake_case，Tauri 自动映射到前端 camelCase（如 `workspace_id` ↔ `workspaceId`）。
- 迁移逻辑必须幂等：对已是最新结构的库重复执行不报错、不重复插入。

## File Structure

**Rust（src-tauri/src/）**

- `db.rs`：`apply_migrations`（表结构迁移 + 默认空间创建）、workspace CRUD、`get_all_agent_settings(workspace_id)` 等作用域化读写、`build_agent_settings`。
- `lib.rs`：`Workspace` / `WorkspaceSummary` 结构体；workspace 与作用域化命令；`agent_chat` 增加 `workspace_id`。
- `agent/types.rs`：`AGENT_SETTING_KEYS` 保持不变（seed 逻辑移入迁移）。
- `agent/mcp/types.rs`：`McpServerConfig` 增加 `workspace_id` 字段。

**前端（src/）**

- `types/index.ts`：新增 `Workspace` / `WorkspaceSummary`；`Conversation` 增加 `workspace_id`；`McpServerConfig` 增加 `workspace_id`。
- `composables/useWorkspaces.ts`（新增）：空间状态单例（列表/当前空间/切换/CRUD）。
- `composables/workspaceUi.ts`（新增）：空间色、路径格式化、表单校验等纯函数。
- `composables/useConversations.ts`：统一数据源 + 按空间过滤 + 迁移 + `moveConversation`。
- `composables/useAgent.ts`：`startAgent` 透传 `workspaceId`。
- `components/WorkspaceSwitcher.vue`（新增）、`components/WorkspaceManager.vue`（新增）。
- `components/Sidebar.vue`、`components/ChatView.vue`、`components/ChatInput.vue`、`components/AgentSettings.vue`、`App.vue`：集成。

---

### Task 1: Rust —— workspaces 表与幂等迁移（含默认工作空间）

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`（`Workspace` 结构体）
- Test: `src-tauri/src/db.rs`（`#[cfg(test)]` 内）

**Interfaces:**
- Produces: `pub struct Workspace { pub id: String, pub name: String, pub path: String, pub archived: bool, pub created_at: i64, pub updated_at: i64 }`（`lib.rs`，`Serialize + Deserialize + Clone`）
- Produces: `pub(crate) fn apply_migrations(conn: &rusqlite::Connection) -> anyhow::Result<()>`（`db.rs` 关联函数，幂等）
- Consumes: 无（本任务自包含）

- [ ] **Step 1: 写失败测试**

在 `src-tauri/src/db.rs` 的 `#[cfg(test)] mod tests` 顶部加 `use rusqlite::Connection;`，并新增：

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test migrate_`
Expected: FAIL（`apply_migrations` 未定义、`workspaces` 表不存在）

- [ ] **Step 3: 实现迁移与建表**

在 `src-tauri/src/db.rs` 中：

1. `init_agent_tables` 的 `agent_settings` 建表语句改为新结构，并**移除该函数内的 seed 循环**：

```rust
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
```

2. `Database::new()` 与 `Database::in_memory()` 在现有建表后追加一行 `Self::apply_migrations(&conn)?;`。

3. 新增迁移函数与列检测辅助函数（放在 `impl Database` 外、`transport_str` 前）：

```rust
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
```

4. 在 `src-tauri/src/lib.rs` 中新增结构体（放在 `Conversation` 之后）：

```rust
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
    pub archived: bool,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test migrate_`
Expected: PASS（3 个迁移测试全部通过）

- [ ] **Step 5: 全量 Rust 测试并提交**

Run: `cd src-tauri && cargo test`
Expected: PASS

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat(workspace): workspaces 表与幂等迁移，创建默认工作空间"
```

---

### Task 2: Rust —— workspace CRUD 命令与默认空间保护

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db.rs`（`#[cfg(test)]` 内）

**Interfaces:**
- Consumes: `Workspace`（Task 1）、`apply_migrations`（Task 1）
- Produces: `pub struct WorkspaceSummary { pub id: String, pub name: String, pub path: String, pub archived: bool, pub created_at: i64, pub updated_at: i64, pub conversation_count: i64 }`（`lib.rs`）
- Produces（db.rs 方法）:
  - `list_workspaces(&self) -> Result<Vec<WorkspaceSummary>>`
  - `get_workspace(&self, id: &str) -> Result<Option<Workspace>>`
  - `create_workspace(&self, id: &str, name: &str, path: &str) -> Result<Workspace>`
  - `update_workspace(&self, id: &str, name: Option<&str>, path: Option<&str>) -> Result<()>`
  - `set_workspace_archived(&self, id: &str, archived: bool) -> Result<()>`（`"default"` 且 `archived` 时返回错误）
  - `delete_workspace(&self, id: &str) -> Result<()>`（`"default"` 拒绝；级联删除该空间会话/设置/MCP）
- Produces（lib.rs 命令）: `list_workspaces` / `create_workspace { name, path }` / `update_workspace { id, name?, path? }` / `set_workspace_archived { id, archived }` / `delete_workspace { id }`

- [ ] **Step 1: 写失败测试**

在 `db.rs` 测试模块新增：

```rust
use crate::agent::mcp::types::McpServerConfig;

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
```

> 注：`delete_workspace_cascades` 依赖 Task 3 的作用域签名，将在 Task 3 编写。

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test workspace_crud_and_default_protection`
Expected: FAIL（方法不存在）

- [ ] **Step 3: 实现 db 方法与命令**

在 `db.rs` 的 `impl Database` 内新增（放在 `get_conversations` 之前）：

```rust
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
    let tx = self.conn.unchecked_transaction()?;
    tx.execute("DELETE FROM conversations WHERE workspace_id = ?1", params![id])
        .context("Failed to delete workspace conversations")?;
    tx.execute("DELETE FROM agent_settings WHERE workspace_id = ?1", params![id])
        .context("Failed to delete workspace settings")?;
    tx.execute("DELETE FROM mcp_servers WHERE workspace_id = ?1", params![id])
        .context("Failed to delete workspace mcp servers")?;
    tx.execute("DELETE FROM workspaces WHERE id = ?1", params![id])
        .context("Failed to delete workspace")?;
    tx.commit().context("Failed to commit workspace delete")
}
```

`db.rs` 顶部 import 增加 `use crate::WorkspaceSummary;`（`Workspace` 已在 `use crate::Conversation` 附近风格相同）。

在 `lib.rs` 中新增结构体与命令：

```rust
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
```

```rust
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
```

在 `lib.rs` 顶部 `use crate::agent::mcp::types::McpServerConfig;` 保持；`invoke_handler` 注册列表追加这 5 个命令。

> 说明：本任务中 `create_conversation` / `set_agent_setting` / `add_mcp_server` 仍是旧签名，Task 3 会改为作用域版本；`delete_workspace_cascades` 测试在 Task 3 编写。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test workspace_crud_and_default_protection`
Expected: PASS

- [ ] **Step 5: 全量测试并提交**

Run: `cd src-tauri && cargo test`
Expected: PASS

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat(workspace): workspace CRUD 命令与默认空间保护"
```

---

### Task 3: Rust —— 会话/设置/MCP 作用域化

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/agent/mcp/types.rs`
- Test: `src-tauri/src/db.rs`（`#[cfg(test)]` 内）

**Interfaces:**
- Consumes: Task 1 迁移（新结构表）、Task 2 的 workspace 方法
- Produces（db.rs 方法，替换旧签名）:
  - `get_conversations(&self, workspace_id: &str) -> Result<Vec<Conversation>>`
  - `create_conversation(&self, workspace_id: &str, title: &str, model: &str) -> Result<Conversation>`
  - `move_conversation(&self, id: &str, workspace_id: &str) -> Result<()>`
  - `get_all_agent_settings(&self, workspace_id: &str) -> Result<HashMap<String, String>>`
  - `set_agent_setting(&self, workspace_id: &str, key: &str, value: &str) -> Result<()>`
  - `list_mcp_servers(&self, workspace_id: &str) -> Result<Vec<McpServerConfig>>`
  - `get_enabled_mcp_servers(&self, workspace_id: &str) -> Result<Vec<McpServerConfig>>`
- Produces: `McpServerConfig` 增加 `pub workspace_id: String`（`#[serde(default = "default_workspace_id")]`）
- Produces（lib.rs 命令）: `get_agent_settings { workspace_id }`、`set_agent_settings { workspace_id, settings }`、`list_mcp_servers { workspace_id }`、`get_conversations { workspace_id }`、`create_conversation { workspace_id, title, model }`、`move_conversation { id, workspace_id }`；`add_mcp_server` 通过 `server.workspace_id` 落库

- [ ] **Step 1: 写失败测试**

在 `db.rs` 测试模块新增：

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test scoped`
Expected: FAIL（`McpServerConfig` 无 `workspace_id` 字段、方法签名不匹配）

- [ ] **Step 3: 实现作用域化**

1. `src-tauri/src/agent/mcp/types.rs`：结构体增加字段：

```rust
    #[serde(default = "default_workspace_id")]
    pub workspace_id: String,
```

并在 `default_timeout` 旁新增：

```rust
fn default_workspace_id() -> String {
    "default".into()
}
```

2. `db.rs` 修改会话方法（SQL 加 `workspace_id`）：

```rust
pub fn get_conversations(&self, workspace_id: &str) -> Result<Vec<Conversation>> {
    let mut stmt = self
        .conn
        .prepare(
            "SELECT id, title, model, created_at, updated_at, messages
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
                workspace_id: "".into(),
            })
        })
        .context("Failed to query conversations")?;
    let mut conversations = Vec::new();
    for row in rows {
        conversations.push(row?);
    }
    Ok(conversations)
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
```

3. `db.rs` 修改设置方法：

```rust
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
```

4. `db.rs` 修改 MCP 方法（`list_mcp_servers` 加过滤；`add_mcp_server` 的 INSERT 加 `workspace_id` 列；`row_to_server` 读取第 9 列）：

```rust
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
```

`add_mcp_server` 的 INSERT 改为：

```rust
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
```

`row_to_server` 中 `workspace_id: row.get(9)?`。

5. `db.rs` 修改 `get_conversation`（SELECT 加 `workspace_id` 并填入返回值）：

```rust
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
```

同时把 `get_conversations` 的 SELECT 改为 `SELECT id, title, model, created_at, updated_at, messages, workspace_id`，row 映射 `workspace_id: row.get(6)?`（不再填空字符串）。

`get_agent_setting` 保留但改为作用域签名（AGENT.md 审批哈希按空间隔离，见 Task 5）：

```rust
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
```

同时更新 `db.rs` 中 `agent_settings_roundtrip` 测试为作用域签名：

```rust
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
```

6. `lib.rs` 的 `Conversation` 结构体增加字段并修改命令：

```rust
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: String,
    pub workspace_id: String,
}
```

```rust
#[tauri::command]
fn get_conversations(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<Conversation>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.get_conversations(&workspace_id).map_err(|e| e.to_string())
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
```

```rust
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
```

```rust
#[tauri::command]
fn list_mcp_servers(
    state: State<AppState>,
    workspace_id: String,
) -> Result<Vec<McpServerConfig>, String> {
    let db = state.db.lock().map_err(|e| e.to_string())?;
    db.list_mcp_servers(&workspace_id).map_err(|e| e.to_string())
}
```

7. `invoke_handler` 注册列表更新为新的命令签名（`get_conversations`、`create_conversation`、`move_conversation`、`get_agent_settings`、`set_agent_settings`、`list_mcp_servers`）。

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test`
Expected: PASS（本任务先不写 `delete_workspace_cascades`，它在 Step 5 后补入——见下方补充步骤）

- [ ] **Step 4b: 补充级联删除测试并跑通**

在测试模块新增：

```rust
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
```

Run: `cd src-tauri && cargo test delete_workspace_cascades`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs src-tauri/src/agent/mcp/types.rs
git commit -m "feat(workspace): 会话/设置/MCP 按工作空间作用域化"
```

---

### Task 4: Rust —— 新建空间复制设置（copy_from）

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db.rs`（`#[cfg(test)]` 内）

**Interfaces:**
- Consumes: Task 3 的 `get_all_agent_settings` / `set_agent_setting` / `list_mcp_servers` / `add_mcp_server`
- Produces: `db.copy_workspace_settings(&self, from_id: &str, to_id: &str) -> Result<()>`；`create_workspace` 命令增加 `copy_from: Option<String>` 参数

- [ ] **Step 1: 写失败测试**

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test copy_workspace_settings`
Expected: FAIL（方法不存在）

- [ ] **Step 3: 实现**

`db.rs` 新增：

```rust
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
```

`lib.rs` 的 `create_workspace` 命令改为：

```rust
#[tauri::command]
fn create_workspace(
    state: State<AppState>,
    name: String,
    path: String,
    copy_from: Option<String>,
) -> Result<Workspace, String> {
    let id = uuid::Uuid::new_v4().to_string();
    let db = state.db.lock().map_err(|e| e.to_string())?;
    let ws = db.create_workspace(&id, &name, &path).map_err(|e| e.to_string())?;
    if let Some(from) = copy_from {
        db.copy_workspace_settings(&from, &id).map_err(|e| e.to_string())?;
    }
    Ok(ws)
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat(workspace): 新建空间支持复制来源设置与 MCP"
```

---

### Task 5: Rust —— agent_chat 按空间加载、归档拒绝、workspace_root 注入

**Files:**
- Modify: `src-tauri/src/db.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/db.rs`（`#[cfg(test)]` 内）

**Interfaces:**
- Consumes: Task 3 作用域方法、`crate::agent::types::load_agent_settings`
- Produces: `db.build_agent_settings(&self, workspace_id: &str) -> Result<(AgentSettings, Vec<McpServerConfig>)>`；`agent_chat` 命令增加 `workspace_id: String` 参数；`run_agent` / `run_agent_inner` / `approve_agent_md_if_needed` 增加 `workspace_id` 参数（AGENT.md 审批哈希按空间隔离）

- [ ] **Step 1: 写失败测试**

```rust
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
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cd src-tauri && cargo test build_agent_settings`
Expected: FAIL（方法不存在）

- [ ] **Step 3: 实现**

`db.rs` 顶部 import 增加：

```rust
use crate::agent::types::{load_agent_settings, AgentSettings};
```

`db.rs` 新增方法（放在 `copy_workspace_settings` 之后）：

```rust
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
```

`lib.rs` 的 `agent_chat` 命令修改为：

```rust
#[tauri::command]
async fn agent_chat(
    app: AppHandle,
    window: tauri::Window,
    state: State<'_, AppState>,
    params: AgentChatParams,
    workspace_id: String,
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
    let (settings, mcp_configs) = {
        let db = state.db.lock().map_err(|e| e.to_string())?;
        db.build_agent_settings(&workspace_id).map_err(|e| e.to_string())?
    };
    let window_label = window.label().to_string();
    tauri::async_runtime::spawn(async move {
        agent::run_agent(app, window_label, params, settings, mcp_configs, workspace_id).await;
    });
    Ok(())
}
```

同时删除 `agent_chat` 中原有的 `get_all_agent_settings` / `get_enabled_mcp_servers` 读取块与 `load_agent_settings` 调用。

`lib.rs` 的 `set_agent_settings` 与 `create_conversation` 命令增加归档检查（归档空间禁写）：

```rust
fn ensure_workspace_active(db: &Database, workspace_id: &str) -> Result<(), String> {
    let ws = db
        .get_workspace(workspace_id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "工作空间不存在".to_string())?;
    if ws.archived {
        return Err("该工作空间已归档，请先恢复后再修改设置".into());
    }
    Ok(())
}
```

`set_agent_settings` 开头与 `create_conversation` 开头各加一行 `ensure_workspace_active(&db, &workspace_id)?;`（在获取 db 锁之后）。

`src-tauri/src/agent/mod.rs` 透传空间 id：

```rust
pub async fn run_agent(
    app: AppHandle,
    window_label: String,
    params: AgentChatParams,
    settings: AgentSettings,
    mcp_configs: Vec<crate::agent::mcp::types::McpServerConfig>,
    workspace_id: String,
) {
    let runtime = AgentRuntime {
        cancellation: CancellationToken::new(),
        usage: Arc::new(UsageCounter::default()),
        window_label: window_label.clone(),
    };
    run_agent_inner(
        &app,
        &window_label,
        &runtime,
        &params,
        &settings,
        mcp_configs,
        &workspace_id,
    )
    .await;
    let state = app.state::<crate::AppState>();
    let mut guard = state.agent.lock().unwrap_or_else(|p| p.into_inner());
    *guard = None;
}
```

`run_agent_inner` 增加 `workspace_id: &str` 参数（签名末尾追加一行 `workspace_id: &str,`），其内部 AGENT.md 审批调用点（当前在 `mod.rs` 约第 157 行）改为：

```rust
if approve_agent_md_if_needed(
    app,
    window_label,
    runtime,
    settings,
    approval,
    workspace_id,
    &hash,
)
.await
```

`approve_agent_md_if_needed` 签名在 `hash: &str` 后追加 `workspace_id: &str` 参数，函数体内改为作用域读写：

```rust
if let Ok(Some(v)) = db.get_agent_setting(workspace_id, "agent.approved_agentmd") {
    ...
}
let cur = db
    .get_agent_setting(workspace_id, "agent.approved_agentmd")
    .ok()
    .flatten()
    .unwrap_or_default();
...
let _ = db.set_agent_setting(workspace_id, "agent.approved_agentmd", &list.join(","));
```

- [ ] **Step 4: 运行测试确认通过**

Run: `cd src-tauri && cargo test`
Expected: PASS（`load_agent_settings` 的旧单元测试仍通过；若 `lib.rs` 不再直接使用 `load_agent_settings`，清理未用 import）

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/db.rs src-tauri/src/lib.rs
git commit -m "feat(workspace): agent_chat 按空间加载设置，归档空间拒绝运行"
```

---

### Task 6: 前端 —— 类型与 useWorkspaces 单例

**Files:**
- Modify: `src/types/index.ts`
- Create: `src/composables/useWorkspaces.ts`
- Test: `src/composables/useWorkspaces.test.ts`（新建）

**Interfaces:**
- Consumes: 无（纯前端）
- Produces:
  - `types/index.ts`: `Workspace { id, name, path, archived, created_at, updated_at }`、`WorkspaceSummary extends Workspace { conversation_count: number }`
  - `useWorkspaces()` 返回: `workspaces: Ref<Workspace[]>`、`currentWorkspaceId: Ref<string>`、`currentWorkspace: ComputedRef<Workspace | null>`、`activeWorkspaces`、`archivedWorkspaces`、`initWorkspaces(): Promise<void>`、`switchWorkspace(id: string): void`、`createWorkspace(input: { name: string; path: string; copyFrom: string | null }): Promise<Workspace | null>`、`renameWorkspace(id: string, name: string): Promise<void>`、`setArchived(id: string, archived: boolean): Promise<void>`、`deleteWorkspace(id: string): Promise<void>`
  - 导出 `buildDefaultWorkspace(): Workspace`（供测试与降级用）

- [ ] **Step 1: 写失败测试**

创建 `src/composables/useWorkspaces.test.ts`（顶部提供 localStorage polyfill；Tauri invoke 被 mock 为失败以走浏览器分支）：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useWorkspaces, buildDefaultWorkspace } from "./useWorkspaces";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("no tauri")),
}));

class MemoryStorage implements Storage {
  private map = new Map<string, string>();
  get length() { return this.map.size; }
  clear() { this.map.clear(); }
  getItem(k: string) { return this.map.get(k) ?? null; }
  key(i: number) { return [...this.map.keys()][i] ?? null; }
  removeItem(k: string) { this.map.delete(k); }
  setItem(k: string, v: string) { this.map.set(k, v); }
}

beforeEach(() => {
  (globalThis as any).localStorage = new MemoryStorage();
});

describe("useWorkspaces（浏览器降级）", () => {
  it("initWorkspaces 首次创建默认工作空间并选中", async () => {
    const ws = useWorkspaces();
    await ws.initWorkspaces();
    expect(ws.workspaces.value).toHaveLength(1);
    expect(ws.currentWorkspace.value?.name).toBe("默认工作空间");
  });

  it("switchWorkspace 持久化当前空间并过滤活跃/归档", async () => {
    const ws = useWorkspaces();
    await ws.initWorkspaces();
    ws.workspaces.value.push({
      id: "w1", name: "项目A", path: "/tmp/a",
      archived: false, created_at: 1, updated_at: 1,
    });
    ws.workspaces.value.push({
      id: "w2", name: "项目B", path: "/tmp/b",
      archived: true, created_at: 2, updated_at: 2,
    });
    ws.switchWorkspace("w1");
    expect(ws.currentWorkspace.value?.id).toBe("w1");
    expect(ws.activeWorkspaces.value.map((w) => w.id)).toEqual(["default", "w1"]);
    expect(ws.archivedWorkspaces.value.map((w) => w.id)).toEqual(["w2"]);
    expect((globalThis as any).localStorage.getItem("chatwhale-active-workspace")).toBe("w1");
  });

  it("buildDefaultWorkspace 返回稳定结构", () => {
    const d = buildDefaultWorkspace();
    expect(d.id).toBe("default");
    expect(d.name).toBe("默认工作空间");
    expect(d.archived).toBe(false);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- useWorkspaces`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现**

`src/types/index.ts` 末尾新增：

```ts
export interface Workspace {
  id: string;
  name: string;
  path: string;
  archived: boolean;
  created_at: number;
  updated_at: number;
}

export interface WorkspaceSummary extends Workspace {
  conversation_count: number;
}
```

`Conversation` 接口增加：

```ts
export interface Conversation {
  id: string;
  title: string;
  model: string;
  created_at: number;
  updated_at: number;
  messages: string;
  workspace_id: string;
}
```

创建 `src/composables/useWorkspaces.ts`：

```ts
import { computed, ref } from "vue";
import type { Workspace, WorkspaceSummary } from "../types";

const STORAGE_KEY = "chatwhale-workspaces";
const ACTIVE_KEY = "chatwhale-active-workspace";
export const DEFAULT_WORKSPACE_ID = "default";

export function buildDefaultWorkspace(): Workspace {
  return {
    id: DEFAULT_WORKSPACE_ID,
    name: "默认工作空间",
    path: "",
    archived: false,
    created_at: Date.now(),
    updated_at: Date.now(),
  };
}

const workspaces = ref<Workspace[]>([]);
const currentWorkspaceId = ref<string>(DEFAULT_WORKSPACE_ID);

export function useWorkspaces() {
  const activeWorkspaces = computed(() => workspaces.value.filter((w) => !w.archived));
  const archivedWorkspaces = computed(() => workspaces.value.filter((w) => w.archived));
  const currentWorkspace = computed(
    () => workspaces.value.find((w) => w.id === currentWorkspaceId.value) ?? null,
  );

  function persistActive() {
    localStorage.setItem(ACTIVE_KEY, currentWorkspaceId.value);
  }

  function saveLocal() {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(workspaces.value));
  }

  function ensureDefault() {
    if (!workspaces.value.some((w) => w.id === DEFAULT_WORKSPACE_ID)) {
      workspaces.value.unshift(buildDefaultWorkspace());
      saveLocal();
    }
  }

  function restoreActive() {
    const saved = localStorage.getItem(ACTIVE_KEY);
    if (saved && workspaces.value.some((w) => w.id === saved)) {
      currentWorkspaceId.value = saved;
    } else {
      currentWorkspaceId.value = DEFAULT_WORKSPACE_ID;
    }
  }

  async function initWorkspaces() {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const list = await invoke<WorkspaceSummary[]>("list_workspaces");
      workspaces.value = list;
      ensureDefault();
      restoreActive();
    } catch {
      const raw = localStorage.getItem(STORAGE_KEY);
      if (raw) {
        try {
          workspaces.value = JSON.parse(raw);
        } catch {
          workspaces.value = [];
        }
      }
      ensureDefault();
      restoreActive();
    }
  }

  function switchWorkspace(id: string) {
    if (!workspaces.value.some((w) => w.id === id)) return;
    currentWorkspaceId.value = id;
    persistActive();
  }

  async function createWorkspace(input: {
    name: string;
    path: string;
    copyFrom: string | null;
  }): Promise<Workspace | null> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const ws = await invoke<Workspace>("create_workspace", {
        name: input.name,
        path: input.path,
        copyFrom: input.copyFrom,
      });
      workspaces.value.push(ws);
      return ws;
    } catch {
      const ws: Workspace = {
        id: crypto.randomUUID(),
        name: input.name,
        path: input.path,
        archived: false,
        created_at: Date.now(),
        updated_at: Date.now(),
      };
      workspaces.value.push(ws);
      saveLocal();
      return ws;
    }
  }

  async function renameWorkspace(id: string, name: string) {
    const ws = workspaces.value.find((w) => w.id === id);
    if (!ws) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("update_workspace", { id, name });
    } catch {
      // 浏览器降级
    }
    ws.name = name;
    ws.updated_at = Date.now();
    saveLocal();
  }

  async function setArchived(id: string, archived: boolean) {
    if (id === DEFAULT_WORKSPACE_ID) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("set_workspace_archived", { id, archived });
    } catch {
      // 浏览器降级
    }
    const ws = workspaces.value.find((w) => w.id === id);
    if (ws) {
      ws.archived = archived;
      ws.updated_at = Date.now();
      saveLocal();
    }
    if (archived && currentWorkspaceId.value === id) {
      switchWorkspace(DEFAULT_WORKSPACE_ID);
    }
  }

  async function deleteWorkspace(id: string) {
    if (id === DEFAULT_WORKSPACE_ID) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_workspace", { id });
    } catch {
      // 浏览器降级
    }
    workspaces.value = workspaces.value.filter((w) => w.id !== id);
    saveLocal();
    if (currentWorkspaceId.value === id) {
      switchWorkspace(DEFAULT_WORKSPACE_ID);
    }
  }

  return {
    workspaces,
    currentWorkspaceId,
    currentWorkspace,
    activeWorkspaces,
    archivedWorkspaces,
    initWorkspaces,
    switchWorkspace,
    createWorkspace,
    renameWorkspace,
    setArchived,
    deleteWorkspace,
  };
}
```

- [ ] **Step 4: 运行测试确认通过**

Run: `npm test -- useWorkspaces`
Expected: PASS

- [ ] **Step 5: typecheck 并提交**

Run: `npm run typecheck`
Expected: 退出码 0

```bash
git add src/types/index.ts src/composables/useWorkspaces.ts src/composables/useWorkspaces.test.ts
git commit -m "feat(workspace): 前端工作空间状态单例 useWorkspaces"
```

---

### Task 7: 前端 —— useConversations 统一数据源与空间过滤

**Files:**
- Modify: `src/composables/useConversations.ts`
- Test: `src/composables/useConversations.test.ts`（新建）

**Interfaces:**
- Consumes: Task 6 的 `DEFAULT_WORKSPACE_ID`
- Produces（`useConversations()` 返回，签名变化）:
  - `loadConversations(workspaceId: string): Promise<void>`
  - `createConversation(title: string, model: string, workspaceId: string): Promise<Conversation>`
  - `updateConversation(id, updates)`（不变）、`deleteConversation(id: string): Promise<void>`、`getConversation(id)`（不变）
  - `moveConversation(id: string, targetWorkspaceId: string): Promise<void>`
  - `conversations`、`groupedConversations`（当前空间子集按时间分组）
- Produces（导出纯函数）: `groupConversationsByTime(convs: Conversation[]): { label: string; items: Conversation[] }[]`

- [ ] **Step 1: 写失败测试**

创建 `src/composables/useConversations.test.ts`：

```ts
import { beforeEach, describe, expect, it, vi } from "vitest";
import { groupConversationsByTime, useConversations } from "./useConversations";
import type { Conversation } from "../types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockRejectedValue(new Error("no tauri")),
}));

class MemoryStorage implements Storage {
  private map = new Map<string, string>();
  get length() { return this.map.size; }
  clear() { this.map.clear(); }
  getItem(k: string) { return this.map.get(k) ?? null; }
  key(i: number) { return [...this.map.keys()][i] ?? null; }
  removeItem(k: string) { this.map.delete(k); }
  setItem(k: string, v: string) { this.map.set(k, v); }
}

function conv(id: string, workspaceId: string, updatedAt: number): Conversation {
  return {
    id, title: "会话", model: "m", created_at: updatedAt,
    updated_at: updatedAt, messages: "[]", workspace_id: workspaceId,
  };
}

beforeEach(() => {
  (globalThis as any).localStorage = new MemoryStorage();
  localStorage.setItem(
    "chatwhale-conversations",
    JSON.stringify([conv("c1", "w1", Date.now()), conv("c2", "w2", Date.now())]),
  );
});

describe("useConversations", () => {
  it("loadConversations 仅加载目标空间会话（浏览器降级）", async () => {
    const c = useConversations();
    await c.loadConversations("w1");
    expect(c.conversations.value.map((x) => x.id)).toEqual(["c1"]);
  });

  it("旧数据缺少 workspace_id 时迁移为 default", async () => {
    localStorage.setItem(
      "chatwhale-conversations",
      JSON.stringify([{ ...conv("c9", "w1", 1), workspace_id: undefined }]),
    );
    const c = useConversations();
    await c.loadConversations("default");
    expect(c.conversations.value[0].workspace_id).toBe("default");
  });

  it("createConversation 绑定当前空间", async () => {
    const c = useConversations();
    const created = await c.createConversation("新对话", "m", "w1");
    expect(created.workspace_id).toBe("w1");
  });

  it("groupConversationsByTime 按时间分组", () => {
    const now = Date.now();
    const groups = groupConversationsByTime([
      conv("a", "w1", now),
      conv("b", "w1", now - 2 * 86400000),
    ]);
    expect(groups[0].label).toBe("今天");
    expect(groups[1].label).toBe("更早");
  });

  it("moveConversation 改变会话归属（浏览器降级）", async () => {
    const c = useConversations();
    await c.loadConversations("w1");
    await c.moveConversation("c1", "w2");
    await c.loadConversations("w2");
    expect(c.conversations.value.map((x) => x.id)).toContain("c1");
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- useConversations`
Expected: FAIL（新签名不存在）

- [ ] **Step 3: 实现**

整体重写 `src/composables/useConversations.ts`：

```ts
import { computed, ref } from "vue";
import type { Conversation } from "../types";
import { DEFAULT_WORKSPACE_ID } from "./useWorkspaces";

const STORAGE_KEY = "chatwhale-conversations";

// Singleton state
const conversations = ref<Conversation[]>([]);

function loadFromStorage(): Conversation[] {
  try {
    const data = localStorage.getItem(STORAGE_KEY);
    const list: Conversation[] = data ? JSON.parse(data) : [];
    let migrated = false;
    for (const c of list) {
      if (!c.workspace_id) {
        c.workspace_id = DEFAULT_WORKSPACE_ID;
        migrated = true;
      }
    }
    if (migrated) localStorage.setItem(STORAGE_KEY, JSON.stringify(list));
    return list;
  } catch {
    return [];
  }
}

function saveToStorage() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(conversations.value));
}

export function groupConversationsByTime(
  convs: Conversation[],
): { label: string; items: Conversation[] }[] {
  const now = Date.now();
  const groups: { label: string; items: Conversation[] }[] = [];
  const today: Conversation[] = [];
  const thisWeek: Conversation[] = [];
  const earlier: Conversation[] = [];
  for (const c of convs) {
    const age = now - c.updated_at;
    if (age < 86400000) today.push(c);
    else if (age < 604800000) thisWeek.push(c);
    else earlier.push(c);
  }
  if (today.length) groups.push({ label: "今天", items: today });
  if (thisWeek.length) groups.push({ label: "本周", items: thisWeek });
  if (earlier.length) groups.push({ label: "更早", items: earlier });
  return groups;
}

export function useConversations() {
  async function loadConversations(workspaceId: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      conversations.value = await invoke<Conversation[]>("get_conversations", {
        workspaceId,
      });
    } catch {
      conversations.value = loadFromStorage().filter(
        (c) => c.workspace_id === workspaceId,
      );
    }
  }

  async function createConversation(
    title: string,
    model: string,
    workspaceId: string,
  ): Promise<Conversation> {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      const conv = await invoke<Conversation>("create_conversation", {
        workspaceId,
        title,
        model,
      });
      conversations.value.unshift(conv);
      return conv;
    } catch {
      const conv: Conversation = {
        id: crypto.randomUUID(),
        title,
        model,
        created_at: Date.now(),
        updated_at: Date.now(),
        messages: "[]",
        workspace_id: workspaceId,
      };
      conversations.value.unshift(conv);
      saveToStorage();
      return conv;
    }
  }

  function updateConversation(
    id: string,
    updates: { title?: string; messages?: string },
  ) {
    const conv = conversations.value.find((c) => c.id === id);
    if (!conv) return;
    if (updates.title !== undefined) conv.title = updates.title;
    if (updates.messages !== undefined) conv.messages = updates.messages;
    conv.updated_at = Date.now();
    saveToStorage();
    // Tauri 模式同步到 SQLite（fire-and-forget，兼容 ChatView 的同步保存回调）
    import("@tauri-apps/api/core")
      .then(({ invoke }) =>
        invoke("update_conversation", {
          id,
          title: updates.title ?? null,
          messages: updates.messages ?? null,
        }),
      )
      .catch(() => {});
  }

  async function deleteConversation(id: string) {
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("delete_conversation", { id });
    } catch {
      // 浏览器降级
    }
    conversations.value = conversations.value.filter((c) => c.id !== id);
    saveToStorage();
  }

  async function moveConversation(id: string, targetWorkspaceId: string) {
    const conv = conversations.value.find((c) => c.id === id);
    if (!conv) return;
    try {
      const { invoke } = await import("@tauri-apps/api/core");
      await invoke("move_conversation", { id, workspaceId: targetWorkspaceId });
    } catch {
      // 浏览器降级：必须基于完整存储列表更新，避免只保存当前空间子集导致数据丢失
      const all = loadFromStorage();
      const target = all.find((c) => c.id === id);
      if (target) {
        target.workspace_id = targetWorkspaceId;
        localStorage.setItem(STORAGE_KEY, JSON.stringify(all));
      }
    }
    conv.workspace_id = targetWorkspaceId;
    conversations.value = conversations.value.filter((c) => c.id !== id);
  }

  function getConversation(id: string): Conversation | undefined {
    return conversations.value.find((c) => c.id === id);
  }

  const groupedConversations = computed(() =>
    groupConversationsByTime(conversations.value),
  );

  return {
    conversations,
    groupedConversations,
    loadConversations,
    createConversation,
    updateConversation,
    deleteConversation,
    moveConversation,
    getConversation,
  };
}
```

> 注：原 `useConversations` 由 `App.vue` / `ChatView.vue` 同步调用（`createConversation` 曾是同步）。本任务先改 composable，Task 8 同步更新调用方，typecheck 会暴露遗漏。

- [ ] **Step 4: 运行测试确认通过**

Run: `npm test -- useConversations`
Expected: PASS

- [ ] **Step 5: 提交**

```bash
git add src/composables/useConversations.ts src/composables/useConversations.test.ts
git commit -m "feat(workspace): 会话数据统一数据源并按工作空间过滤"
```

---

### Task 8: 前端 —— useAgent 透传空间 + App/ChatView 集成切换时序

**Files:**
- Modify: `src/composables/useAgent.ts`
- Modify: `src/App.vue`
- Modify: `src/components/ChatView.vue`
- Modify: `src/components/Sidebar.vue`（先只接事件，UI 在 Task 9）
- Test: `src/composables/useAgent.test.ts`（追加）

**Interfaces:**
- Consumes: Task 6 `useWorkspaces`、Task 7 `useConversations` 新签名
- Produces:
  - `useAgent` 的 `startAgent(params: AgentChatParams, workspaceId: string)`（invoke 带 `workspaceId`）
  - `ChatView` 新增 prop `workspaceId: string`；`App.vue` 新增事件流 `select-workspace`、`open-workspace-manager`

- [ ] **Step 1: 写失败测试**

在 `src/composables/useAgent.test.ts` 追加：

```ts
import { describe, expect, it, vi, beforeEach } from "vitest";

const invokeMock = vi.fn();
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invokeMock(...a) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

import { ref } from "vue";
import { useAgent } from "./useAgent";

describe("useAgent startAgent workspaceId", () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it("invoke agent_chat 时携带 workspaceId", async () => {
    const messages = ref([]);
    const agent = useAgent(messages as any, () => {});
    invokeMock.mockRejectedValue(new Error("stop"));
    await agent.startAgent({} as any, "w1");
    expect(invokeMock).toHaveBeenCalledWith("agent_chat", {
      params: {},
      workspaceId: "w1",
    });
  });
});
```

> 若现有 `useAgent.test.ts` 结构不同，将上述用例合并进该文件的对应 `describe`，保留原有断言。

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- useAgent`
Expected: FAIL（`startAgent` 第二参数未透传）

- [ ] **Step 3: 实现**

`src/composables/useAgent.ts` 修改 `startAgent`：

```ts
async function startAgent(params: AgentChatParams, workspaceId: string) {
  ...
  try {
    await invoke("agent_chat", { params, workspaceId });
  } catch (err) {
    ...
  }
}
```

`src/components/ChatView.vue`：

- props 增加 `workspaceId: string`；
- `startAgent` 调用处改为 `await startAgent({ ... }, props.workspaceId)`；
- 新增 watch：切换空间前保存当前会话：

```ts
watch(
  () => props.workspaceId,
  () => {
    if (props.convId && messages.value.length > 0) {
      saveMessages();
    }
  },
);
```

`src/App.vue`：

```ts
import { useWorkspaces } from "./composables/useWorkspaces";

const {
  currentWorkspace,
  activeWorkspaces,
  archivedWorkspaces,
  initWorkspaces,
  switchWorkspace,
} = useWorkspaces();

const {
  groupedConversations,
  conversations,
  loadConversations,
  createConversation,
} = useConversations();

const showWorkspaceManager = ref(false);

async function selectWorkspace(id: string) {
  switchWorkspace(id);
  await loadConversations(id);
  // 会话仍属于目标空间则保留，否则回到空态
  currentConvId.value = conversations.value.some((c) => c.id === currentConvId.value)
    ? currentConvId.value
    : null;
}

async function newConversation() {
  const ws = currentWorkspace.value;
  if (!ws || ws.archived) return;
  const conv = await createConversation("新对话", currentModel.value, ws.id);
  currentConvId.value = conv.id;
}
```

模板中 `Sidebar` 增加事件绑定与 props：

```html
<Sidebar
  :current-workspace="currentWorkspace"
  :active-workspaces="activeWorkspaces"
  :archived-workspaces="archivedWorkspaces"
  ...
  @select-workspace="selectWorkspace"
  @open-workspace-manager="showWorkspaceManager = true"
/>
<ChatView
  :key="currentConvId"
  :conv-id="currentConvId"
  :model="currentModel"
  :workspace-id="currentWorkspace?.id ?? 'default'"
/>
```

`onMounted` 中 `initWorkspaces` 完成后加载会话：

```ts
onMounted(async () => {
  ...
  await initWorkspaces();
  await loadConversations(currentWorkspace.value?.id ?? "default");
});
```

`Sidebar.vue` 本任务仅增加 props 声明（`currentWorkspace`、`activeWorkspaces`、`archivedWorkspaces`、`isAgentRunning`）与 emits 声明（`selectWorkspace`、`openWorkspaceManager`），不渲染新组件，保证模板不报错；实际 UI 在 Task 9。

- [ ] **Step 4: 运行测试与 typecheck**

Run: `npm test && npm run typecheck`
Expected: 全部 PASS / 退出码 0

- [ ] **Step 5: 提交**

```bash
git add src/composables/useAgent.ts src/composables/useAgent.test.ts src/App.vue src/components/ChatView.vue src/components/Sidebar.vue
git commit -m "feat(workspace): Agent 透传空间 id，App 集成初始化与切换时序"
```

---

### Task 9: 前端 —— WorkspaceSwitcher 组件与 Sidebar 集成

**Files:**
- Create: `src/composables/workspaceUi.ts`
- Create: `src/components/WorkspaceSwitcher.vue`
- Modify: `src/components/Sidebar.vue`
- Modify: `src/App.vue`
- Test: `src/composables/workspaceUi.test.ts`（新建）

**Interfaces:**
- Consumes: Task 6 `useWorkspaces` 状态、Task 2 的 `WorkspaceSummary`
- Produces:
  - `workspaceUi.ts`: `WORKSPACE_COLORS: string[]`、`workspaceColor(id: string): string`、`formatWorkspacePath(path: string): string`、`validateWorkspaceName(name: string): boolean`
  - `WorkspaceSwitcher.vue` props: `{ currentWorkspace: Workspace | null; active: WorkspaceSummary[]; archived: WorkspaceSummary[]; isAgentRunning: boolean }`；emits: `select(id: string)`、`openManager: []`、`newWorkspace: []`
  - `Sidebar.vue` 新增 emits: `selectWorkspace`、`openWorkspaceManager`；新增 props（透传）

- [ ] **Step 1: 写失败测试**

创建 `src/composables/workspaceUi.test.ts`：

```ts
import { describe, expect, it } from "vitest";
import {
  WORKSPACE_COLORS,
  formatWorkspacePath,
  validateWorkspaceName,
  workspaceColor,
} from "./workspaceUi";

describe("workspaceUi", () => {
  it("默认空间使用固定鲸青色", () => {
    expect(workspaceColor("default")).toBe(WORKSPACE_COLORS[0]);
  });

  it("颜色分配稳定且落在调色板内", () => {
    const a = workspaceColor("w-abc");
    const b = workspaceColor("w-abc");
    expect(a).toBe(b);
    expect(WORKSPACE_COLORS).toContain(a);
  });

  it("空路径显示未配置目录", () => {
    expect(formatWorkspacePath("")).toBe("未配置目录");
    expect(formatWorkspacePath("/tmp/a")).toBe("/tmp/a");
  });

  it("空间名校验拒绝空白", () => {
    expect(validateWorkspaceName("  ")).toBe(false);
    expect(validateWorkspaceName("项目A")).toBe(true);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- workspaceUi`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现**

创建 `src/composables/workspaceUi.ts`：

```ts
export const WORKSPACE_COLORS = [
  "#4fc3b4", "#d4745c", "#74fcc0", "#5068c8",
  "#2d9b8e", "#d4a45c", "#a474d4", "#5c8ad4",
];

export function workspaceColor(id: string): string {
  if (id === "default") return WORKSPACE_COLORS[0];
  let h = 0;
  for (const ch of id) {
    h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  }
  return WORKSPACE_COLORS[h % WORKSPACE_COLORS.length];
}

export function formatWorkspacePath(path: string): string {
  return path ? path : "未配置目录";
}

export function validateWorkspaceName(name: string): boolean {
  return name.trim().length > 0;
}
```

创建 `src/components/WorkspaceSwitcher.vue`：

```vue
<script setup lang="ts">
import { ref } from "vue";
import type { Workspace, WorkspaceSummary } from "../types";
import { formatWorkspacePath, workspaceColor } from "../composables/workspaceUi";

defineProps<{
  currentWorkspace: Workspace | null;
  active: WorkspaceSummary[];
  archived: WorkspaceSummary[];
  isAgentRunning: boolean;
}>();

const emit = defineEmits<{
  select: [id: string];
  openManager: [];
  newWorkspace: [];
}>();

const open = ref(false);

function toggle() {
  if (open.value) {
    open.value = false;
    return;
  }
  open.value = true;
}

function pick(id: string) {
  open.value = false;
  emit("select", id);
}
</script>

<template>
  <div class="ws-switcher">
    <button
      class="ws-trigger"
      :disabled="isAgentRunning"
      :title="isAgentRunning ? 'Agent 正在运行，请稍后再切换' : '切换工作空间'"
      @click="toggle"
    >
      <span
        class="ws-dot"
        :style="{ background: workspaceColor(currentWorkspace?.id ?? 'default') }"
      ></span>
      <span class="ws-meta">
        <span class="ws-name">{{ currentWorkspace?.name ?? "未选择工作空间" }}</span>
        <span class="ws-path">{{ formatWorkspacePath(currentWorkspace?.path ?? "") }}</span>
      </span>
      <span class="ws-chevron">▾</span>
    </button>

    <div v-if="open" class="ws-popover">
      <div class="ws-group-title">工作空间</div>
      <div
        v-for="w in active"
        :key="w.id"
        class="ws-item"
        :class="{ active: w.id === currentWorkspace?.id }"
        @click="pick(w.id)"
      >
        <span class="ws-item-dot" :style="{ background: workspaceColor(w.id) }"></span>
        <span class="ws-item-text">
          <span class="ws-item-name">{{ w.name }}</span>
          <span class="ws-item-path">{{ formatWorkspacePath(w.path) }} · {{ w.conversation_count }} 会话</span>
        </span>
      </div>

      <template v-if="archived.length">
        <div class="ws-group-title">已归档</div>
        <div
          v-for="w in archived"
          :key="w.id"
          class="ws-item"
          @click="pick(w.id)"
        >
          <span class="ws-item-dot" :style="{ background: workspaceColor(w.id) }"></span>
          <span class="ws-item-text">
            <span class="ws-item-name">📦 {{ w.name }}</span>
            <span class="ws-item-path">{{ formatWorkspacePath(w.path) }} · 已归档</span>
          </span>
        </div>
      </template>

      <div class="ws-actions">
        <button class="ws-btn" @click="emit('newWorkspace')">+ 新建工作空间</button>
        <button class="ws-btn" @click="emit('openManager')">管理空间…</button>
      </div>
    </div>

    <div v-if="open" class="ws-backdrop" @click="open = false"></div>
  </div>
</template>

<style scoped>
.ws-switcher { position: relative; padding: 10px 12px 4px; }
.ws-trigger {
  width: 100%; display: flex; align-items: center; gap: 8px;
  padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg-card); color: var(--text-primary); cursor: pointer;
  font-family: var(--font-sans); text-align: left;
}
.ws-trigger:disabled { opacity: 0.5; cursor: not-allowed; }
.ws-trigger:hover:not(:disabled) { border-color: var(--border-active); }
.ws-dot, .ws-item-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
.ws-meta { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.ws-name { font-size: 13px; font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-chevron { color: var(--text-muted); font-size: 11px; }
.ws-popover {
  position: absolute; left: 12px; right: 12px; top: calc(100% - 2px); z-index: 60;
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius);
  box-shadow: 0 8px 24px rgba(0,0,0,0.25); max-height: 320px; overflow-y: auto;
  padding: 8px;
}
.ws-backdrop { position: fixed; inset: 0; z-index: 55; }
.ws-popover { z-index: 60; }
.ws-group-title {
  font-size: 11px; font-weight: 600; color: var(--text-muted);
  padding: 6px 8px 4px; text-transform: uppercase; letter-spacing: 0.5px;
}
.ws-item {
  display: flex; align-items: center; gap: 8px; padding: 7px 8px;
  border-radius: var(--radius-sm); cursor: pointer;
}
.ws-item:hover { background: var(--bg-hover); }
.ws-item.active { background: var(--accent-bg); }
.ws-item-text { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.ws-item-name { font-size: 13px; color: var(--text-primary); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-item-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-actions { display: flex; gap: 6px; padding: 8px 4px 4px; border-top: 1px solid var(--border); margin-top: 6px; }
.ws-btn {
  flex: 1; padding: 6px 8px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: transparent; color: var(--text-secondary); font-size: 12px; cursor: pointer;
  font-family: var(--font-sans);
}
.ws-btn:hover { background: var(--bg-hover); color: var(--text-primary); }
</style>
```

`src/components/Sidebar.vue`：props 增加 `currentWorkspace`、`activeWorkspaces`、`archivedWorkspaces`、`isAgentRunning`；emits 增加 `selectWorkspace`、`openWorkspaceManager`、`newWorkspace`（与现有"新建对话"的 `newConversation` emit 并存）；模板品牌区后插入：

```html
<WorkspaceSwitcher
  :current-workspace="currentWorkspace"
  :active="activeWorkspaces"
  :archived="archivedWorkspaces"
  :is-agent-running="isAgentRunning"
  @select="emit('selectWorkspace', $event)"
  @open-manager="emit('openWorkspaceManager')"
  @new-workspace="emit('newWorkspace')"
/>
```

`Sidebar.vue` 空态文案改为"该工作空间暂无对话"；会话项左侧加色点：

```html
<span
  class="conv-item-icon"
  :style="{ color: workspaceColor(currentWorkspace?.id ?? 'default') }"
>●</span>
```

`Sidebar.vue` 引入组件与工具函数：

```ts
import WorkspaceSwitcher from "./WorkspaceSwitcher.vue";
import { workspaceColor } from "../composables/workspaceUi";
```

会话项增加"更多"菜单（移动到其他空间 / 删除）。`Sidebar.vue` 的 emits 增加 `moveConversation: [id: string, target: string]`、`deleteConversation: [id: string]`；会话项模板改为：

```html
<div
  v-for="conv in group.items"
  :key="conv.id"
  class="conv-item"
  :class="{ active: conv.id === currentConvId }"
  @click="emit('selectConversation', conv.id)"
>
  <span
    class="conv-item-icon"
    :style="{ color: workspaceColor(currentWorkspace?.id ?? 'default') }"
  >●</span>
  <span class="conv-item-text">{{ conv.title }}</span>
  <button class="conv-more" @click.stop="openMenuFor = openMenuFor === conv.id ? null : conv.id">⋮</button>
  <div v-if="openMenuFor === conv.id" class="conv-menu" @click.stop>
    <div
      v-for="w in activeWorkspaces"
      :key="w.id"
      class="conv-menu-item"
      @click="emit('moveConversation', conv.id, w.id)"
    >移动到「{{ w.name }}」</div>
    <div class="conv-menu-item danger" @click="emit('deleteConversation', conv.id)">删除</div>
  </div>
</div>
```

`Sidebar.vue` script 增加 `const openMenuFor = ref<string | null>(null);`，样式新增：

```css
.conv-item { position: relative; }
.conv-more {
  margin-left: auto; width: 20px; height: 20px; border: none; border-radius: 4px;
  background: transparent; color: var(--text-muted); cursor: pointer; opacity: 0;
  flex-shrink: 0; line-height: 1;
}
.conv-item:hover .conv-more { opacity: 1; }
.conv-menu {
  position: absolute; right: 8px; top: calc(100% - 2px); z-index: 70;
  background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-sm);
  box-shadow: 0 6px 18px rgba(0,0,0,0.2); padding: 4px; min-width: 150px;
}
.conv-menu-item {
  padding: 6px 10px; border-radius: 4px; font-size: 12px; cursor: pointer;
  color: var(--text-secondary); white-space: nowrap;
}
.conv-menu-item:hover { background: var(--bg-hover); color: var(--text-primary); }
.conv-menu-item.danger { color: #e05b5b; }
```

`App.vue` 从 `useConversations()` 解构 `moveConversation`、`deleteConversation` 并绑定事件：

```html
@move-conversation="(id, target) => moveConversation(id, target)"
@delete-conversation="deleteConversation"
```

`App.vue`：`newWorkspace` 事件绑定 `showWorkspaceManager = true`；`Sidebar` 的 `:is-agent-running` 本任务先传 `false`，Task 11 接入 `agentRunning` 状态（由 `ChatView` 通过 `agent-running-change` 事件上报）。

- [ ] **Step 4: 运行测试与 typecheck**

Run: `npm test && npm run typecheck`
Expected: 全部 PASS / 退出码 0

- [ ] **Step 5: 提交**

```bash
git add src/composables/workspaceUi.ts src/composables/workspaceUi.test.ts src/components/WorkspaceSwitcher.vue src/components/Sidebar.vue src/App.vue
git commit -m "feat(workspace): 侧边栏工作空间切换器"
```

---

### Task 10: 前端 —— WorkspaceManager 空间管理弹窗

**Files:**
- Create: `src/components/WorkspaceManager.vue`
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: Task 6 `useWorkspaces` CRUD、Task 2 `WorkspaceSummary`
- Produces:
  - `WorkspaceManager.vue` props: `{ workspaces: WorkspaceSummary[]; currentId: string; tauriAvailable: boolean }`；emits: `close: []`、`refresh: []`、`openAgentSettings: [workspaceId: string]`
  - `App.vue` 渲染 `<WorkspaceManager v-if="showWorkspaceManager" ... @close="showWorkspaceManager = false" @refresh="..." />`

- [ ] **Step 1: 实现组件**（组件渲染依赖浏览器/桌面环境，按项目现有测试风格不写组件单测；纯逻辑已在 workspaceUi 覆盖）

创建 `src/components/WorkspaceManager.vue`：

```vue
<script setup lang="ts">
import { ref } from "vue";
import type { WorkspaceSummary } from "../types";
import { formatWorkspacePath, validateWorkspaceName } from "../composables/workspaceUi";

const props = defineProps<{
  workspaces: WorkspaceSummary[];
  currentId: string;
}>();

const emit = defineEmits<{
  close: [];
  refresh: [];
  openAgentSettings: [workspaceId: string];
}>();

const newName = ref("");
const newPath = ref("");
const copyFrom = ref<string | null>(null);
const errorMsg = ref("");
const deleting = ref<WorkspaceSummary | null>(null);
const deleteConfirm = ref("");

function copyOptions(): { id: string; label: string }[] {
  const list = [
    { id: "__none__", label: "不复制（使用默认值）" },
    ...props.workspaces
      .filter((w) => !w.archived)
      .map((w) => ({ id: w.id, label: w.name })),
  ];
  return list;
}

async function pickDirectory() {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") newPath.value = dir;
  } catch {
    // 浏览器模式手输路径
  }
}

async function createWorkspace() {
  const name = newName.value.trim();
  if (!validateWorkspaceName(name)) {
    errorMsg.value = "请输入空间名称";
    return;
  }
  errorMsg.value = "";
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  const ws = await useWorkspaces().createWorkspace({
    name,
    path: newPath.value.trim(),
    copyFrom: copyFrom.value === "__none__" ? null : copyFrom.value,
  });
  if (ws) {
    newName.value = "";
    newPath.value = "";
    copyFrom.value = null;
    emit("refresh");
  }
}

async function toggleArchived(w: WorkspaceSummary) {
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().setArchived(w.id, !w.archived);
  emit("refresh");
}

async function rename(w: WorkspaceSummary) {
  const name = window.prompt("重命名工作空间", w.name);
  if (!name || !validateWorkspaceName(name)) return;
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().renameWorkspace(w.id, name.trim());
  emit("refresh");
}

async function confirmDelete() {
  if (!deleting.value) return;
  if (deleteConfirm.value !== deleting.value.name) return;
  const { useWorkspaces } = await import("../composables/useWorkspaces");
  await useWorkspaces().deleteWorkspace(deleting.value.id);
  deleting.value = null;
  deleteConfirm.value = "";
  emit("refresh");
}
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-panel">
      <div class="settings-header">
        <h2>工作空间管理</h2>
        <button class="close-btn" @click="emit('close')">✕</button>
      </div>
      <div class="settings-body">
        <div class="settings-section">新建工作空间</div>
        <div class="setting-group">
          <label class="setting-label">名称</label>
          <input v-model="newName" class="setting-input" placeholder="项目名称" />
        </div>
        <div class="setting-group">
          <label class="setting-label">工作目录</label>
          <div class="dir-row">
            <input v-model="newPath" class="setting-input" placeholder="/path/to/project" />
            <button class="btn-secondary" @click="pickDirectory">选择</button>
          </div>
        </div>
        <div class="setting-group">
          <label class="setting-label">复制设置来源</label>
          <select v-model="copyFrom" class="setting-input">
            <option v-for="o in copyOptions()" :key="o.id" :value="o.id">{{ o.label }}</option>
          </select>
        </div>
        <div v-if="errorMsg" class="agent-error">{{ errorMsg }}</div>
        <button class="btn-primary" @click="createWorkspace">创建工作空间</button>

        <div class="settings-section">空间列表</div>
        <div class="ws-list">
          <div v-for="w in workspaces" :key="w.id" class="ws-row">
            <div class="ws-row-main">
              <span class="ws-row-name">{{ w.archived ? "📦 " : "" }}{{ w.name }}</span>
              <span class="ws-row-path">{{ formatWorkspacePath(w.path) }} · {{ w.conversation_count }} 会话</span>
            </div>
            <div class="ws-row-actions">
              <button class="btn-secondary" @click="rename(w)">重命名</button>
              <button class="btn-secondary" @click="emit('openAgentSettings', w.id)">Agent 设置</button>
              <button
                v-if="w.id !== 'default'"
                class="btn-secondary"
                @click="toggleArchived(w)"
              >{{ w.archived ? "恢复" : "归档" }}</button>
              <button
                v-if="w.id !== 'default'"
                class="btn-secondary danger"
                @click="deleting = w; deleteConfirm = ''"
              >彻底删除</button>
            </div>
          </div>
        </div>
      </div>

      <div v-if="deleting" class="mcp-editor-overlay" @click.self="deleting = null">
        <div class="mcp-editor">
          <h3>彻底删除「{{ deleting.name }}」</h3>
          <p class="delete-warn">
            将永久删除该空间的 {{ deleting.conversation_count }} 个会话及其全部设置，且不可恢复。
            请输入空间名称「{{ deleting.name }}」确认：
          </p>
          <input v-model="deleteConfirm" class="setting-input" :placeholder="deleting.name" />
          <div class="approval-actions">
            <button class="btn-primary danger" :disabled="deleteConfirm !== deleting.name" @click="confirmDelete">确认删除</button>
            <button class="btn-secondary" @click="deleting = null">取消</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.4);
  display: flex; align-items: center; justify-content: center; z-index: 100;
}
.settings-panel {
  width: 620px; max-height: 84vh; background: var(--bg-card);
  border: 1px solid var(--border); border-radius: var(--radius-lg);
  overflow: hidden; display: flex; flex-direction: column;
}
.settings-header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 16px 20px; border-bottom: 1px solid var(--border);
}
.settings-header h2 { font-size: 15px; font-weight: 600; }
.close-btn {
  width: 28px; height: 28px; border-radius: var(--radius-sm); border: none;
  background: transparent; color: var(--text-muted); cursor: pointer; font-size: 14px;
}
.settings-body { padding: 16px 20px; overflow-y: auto; display: flex; flex-direction: column; gap: 12px; }
.settings-section {
  font-size: 12px; font-weight: 700; color: var(--accent);
  margin-top: 8px; text-transform: uppercase; letter-spacing: 0.5px;
}
.setting-group { display: flex; flex-direction: column; gap: 6px; }
.setting-label { font-size: 12px; font-weight: 600; color: var(--text-secondary); }
.setting-input {
  padding: 8px 12px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--bg-input); color: var(--text-primary); font-size: 13px;
  font-family: var(--font-mono); outline: none; width: 100%;
}
.dir-row { display: flex; gap: 8px; }
.dir-row .setting-input { flex: 1; }
.btn-primary, .btn-secondary {
  padding: 8px 16px; border-radius: var(--radius-sm); font-size: 13px; cursor: pointer; border: none;
  font-family: var(--font-sans);
}
.btn-primary { background: var(--accent); color: var(--bg-primary); }
.btn-primary:hover { opacity: 0.85; }
.btn-secondary { background: var(--bg-hover); color: var(--text-secondary); }
.btn-secondary:hover { background: var(--border); color: var(--text-primary); }
.danger { background: #e05b5b; color: #fff; }
.agent-error { color: #e05b5b; font-size: 12px; }
.ws-list { display: flex; flex-direction: column; gap: 8px; }
.ws-row {
  border: 1px solid var(--border); border-radius: var(--radius-sm);
  padding: 10px 12px; display: flex; align-items: center; justify-content: space-between; gap: 8px;
}
.ws-row-main { display: flex; flex-direction: column; min-width: 0; }
.ws-row-name { font-size: 13px; font-weight: 600; }
.ws-row-path { font-size: 11px; color: var(--text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.ws-row-actions { display: flex; gap: 6px; flex-shrink: 0; }
.mcp-editor-overlay {
  position: fixed; inset: 0; background: rgba(0,0,0,0.5);
  display: flex; align-items: center; justify-content: center; z-index: 110;
}
.mcp-editor {
  width: 460px; background: var(--bg-card); border: 1px solid var(--border);
  border-radius: var(--radius-lg); padding: 20px;
  display: flex; flex-direction: column; gap: 10px;
}
.delete-warn { font-size: 13px; color: var(--text-secondary); }
.approval-actions { display: flex; gap: 8px; margin-top: 10px; }
</style>
```

`App.vue` 中挂载：

```html
<WorkspaceManager
  v-if="showWorkspaceManager"
  :workspaces="workspaceSummaries"
  :current-id="currentWorkspace?.id ?? 'default'"
  @close="showWorkspaceManager = false"
  @refresh="refreshAfterManage"
  @open-agent-settings="openAgentSettingsFor"
/>
```

`App.vue` 计算 `workspaceSummaries`（把 `workspaces` 补上 `conversation_count`，Tauri 模式由 `list_workspaces` 返回，浏览器降级为 0）：

```ts
const workspaceSummaries = computed(() =>
  workspaces.value.map((w) => ({
    ...w,
    conversation_count: 0,
  })),
);
```

`refreshAfterManage` 重新拉取空间与会话：

```ts
async function refreshAfterManage() {
  await initWorkspaces();
  await loadConversations(currentWorkspace.value?.id ?? "default");
}
```

`openAgentSettingsFor(workspaceId)`：设置 `agentSettingsWorkspaceId`、`showAgentSettings = true`。本任务只在 `App.vue` 增加 `const agentSettingsWorkspaceId = ref<string | null>(null);` 状态，**不**向 `AgentSettings` 模板传新 props（组件尚无这些 props，vue-tsc 会报错）；Task 11 实现 `AgentSettings` 的 `workspaceId` / `workspaceName` props 后再在模板传入。

- [ ] **Step 2: typecheck 与测试**

Run: `npm test && npm run typecheck`
Expected: 全部 PASS / 退出码 0

- [ ] **Step 3: 提交**

```bash
git add src/components/WorkspaceManager.vue src/App.vue
git commit -m "feat(workspace): 工作空间管理弹窗（新建/归档/删除）"
```

---

### Task 11: 前端 —— ChatView 归档只读 + AgentSettings 作用域化 + ChatInput 禁用

**Files:**
- Create: `src/composables/agentSettingsFields.ts`
- Modify: `src/components/ChatView.vue`
- Modify: `src/components/ChatInput.vue`
- Modify: `src/components/AgentSettings.vue`
- Modify: `src/components/WorkspaceSwitcher.vue`（增加 `pathMissing` prop）
- Modify: `src/types/index.ts`（`McpServerConfig.workspace_id`）
- Modify: `src/composables/useAgentSettings.test.ts`（断言 SETTING_FIELDS）
- Modify: `src/App.vue`

**Interfaces:**
- Consumes: Task 3 Rust 命令签名、Task 6-9 前端状态
- Produces:
  - `ChatView` 新增 prop `workspaceArchived: boolean` 与 emit `agentRunningChange: [running: boolean]`；`ChatInput` 新增 prop `disabled?: boolean`
  - `AgentSettings` 新增 props `{ workspaceId: string; workspaceName: string }`；所有 invoke 带 `workspace_id`；MCP server 对象含 `workspace_id`
  - `src/composables/agentSettingsFields.ts` 导出 `SETTING_FIELDS: SettingField[]`（不含 `agent.workspace_root`）
  - `WorkspaceSwitcher` 新增 prop `pathMissing?: boolean`（路径前显示 ⚠）
  - `types/index.ts`：`McpServerConfig` 增加 `workspace_id: string`

- [ ] **Step 1: 写失败测试**

创建 `src/composables/agentSettingsFields.ts` 前先写测试。在 `src/composables/useAgentSettings.test.ts` 追加：

```ts
import { SETTING_FIELDS } from "./agentSettingsFields";

describe("AgentSettings 设置字段", () => {
  it("工作目录字段已退役（由工作空间管理）", () => {
    expect(SETTING_FIELDS.some((f) => f.key === "agent.workspace_root")).toBe(false);
  });

  it("设置字段非空且键唯一", () => {
    expect(SETTING_FIELDS.length).toBeGreaterThan(0);
    const keys = SETTING_FIELDS.map((f) => f.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run: `npm test -- useAgentSettings`
Expected: FAIL（`agentSettingsFields` 模块不存在）

- [ ] **Step 3: 实现**

1. 创建 `src/composables/agentSettingsFields.ts`：

```ts
export interface SettingField {
  key: string;
  label: string;
  type: "text" | "number" | "select" | "textarea";
  options?: string[];
}

export const SETTING_FIELDS: SettingField[] = [
  { key: "agent.skills_dir", label: "Skills 目录（全局）", type: "text" },
  { key: "agent.command_approval", label: "命令审批策略", type: "select", options: ["always", "whitelist", "never"] },
  { key: "agent.max_iterations", label: "最大工具循环次数", type: "number" },
  { key: "agent.llm_timeout", label: "LLM 超时（秒）", type: "number" },
  { key: "agent.command_timeout", label: "命令超时（秒）", type: "number" },
  { key: "agent.approval_timeout", label: "审批超时（秒）", type: "number" },
  { key: "agent.max_result_bytes", label: "工具结果上限（字节）", type: "number" },
  { key: "agent.command_whitelist", label: "命令白名单（JSON）", type: "textarea" },
  { key: "agent.sensitive_paths", label: "敏感路径扩展（JSON，glob）", type: "textarea" },
];
```

2. `src/types/index.ts`：`McpServerConfig` 增加：

```ts
export interface McpServerConfig {
  id: string;
  workspace_id: string;
  // ...其余字段不变
}
```

3. `src/components/AgentSettings.vue`：

- 删除原内联 `SETTING_FIELDS` 常量，改为 `import { SETTING_FIELDS } from "../composables/agentSettingsFields";`；
- 增加 props：

```ts
const props = defineProps<{
  workspaceId: string;
  workspaceName: string;
}>();
```

- `load()` / `save()` / `list_mcp_servers` 等 invoke 加 `workspaceId: props.workspaceId`：

```ts
settings.value = await invoke<Record<string, string>>("get_agent_settings", {
  workspaceId: props.workspaceId,
});
mcpServers.value = await invoke<McpServerConfig[]>("list_mcp_servers", {
  workspaceId: props.workspaceId,
});
await invoke("set_agent_settings", {
  workspaceId: props.workspaceId,
  settings: normalizeAgentSettings(settings.value),
});
```

- `newServer()` / `editServer()` 中对象增加 `workspace_id: props.workspaceId`；标题改为 `<h2>Agent 设置 · {{ workspaceName }}</h2>`；移除"工作目录 (workspace)"输入组与 `pickDirectory` 对 `agent.workspace_root` 的专用 placeholder。

4. `src/components/ChatInput.vue`：props 增加 `disabled?: boolean`；textarea、发送按钮、Agent 开关按钮加 `:disabled` 属性。

5. `src/components/ChatView.vue`：

- props 增加 `workspaceArchived: boolean`；
- emits 增加 `agentRunningChange: [running: boolean]`，并上报运行状态：

```ts
watch(isAgentRunning, (running) => {
  emit("agentRunningChange", running);
});
```

- `toggleAgentMode` 与 `handleSend` 入口增加 `if (props.workspaceArchived) return;`；
- 模板在 `chat-area` 前插入横幅：

```html
<div v-if="workspaceArchived" class="archived-banner">
  此工作空间已归档，可查看历史会话；继续对话请恢复工作空间
</div>
```

- `ChatInput` 传 `:disabled="workspaceArchived"`；新增样式 `.archived-banner { padding: 8px 24px; background: var(--accent-bg); color: var(--accent); font-size: 12px; }`。

6. `src/components/WorkspaceSwitcher.vue`：props 增加 `pathMissing?: boolean`，路径渲染：

```html
<span class="ws-path">
  {{ pathMissing ? "⚠ " : "" }}{{ formatWorkspacePath(currentWorkspace?.path ?? "") }}
</span>
```

7. `src/App.vue`：

- `AgentSettings` 传 `:workspace-id="agentSettingsWorkspaceId ?? currentWorkspace?.id ?? 'default'"` 与 `:workspace-name="...?.name ?? ''"`；
- `ChatView` 传 `:workspace-archived="currentWorkspace?.archived ?? false"`，并监听运行状态：`@agent-running-change="(v) => (agentRunning = v)"`，`App.vue` 增加 `const agentRunning = ref(false);`；`Sidebar` 的 `:is-agent-running` 绑定 `agentRunning`；
- 目录不可达警示：`App.vue` 增加 `pathMissing` ref 与 watch（Tauri 模式用 `@tauri-apps/plugin-fs` 的 `exists`，浏览器模式跳过）：

```ts
const pathMissing = ref(false);
watch(
  () => currentWorkspace.value?.path,
  async (p) => {
    if (!p) {
      pathMissing.value = false;
      return;
    }
    try {
      const { exists } = await import("@tauri-apps/plugin-fs");
      pathMissing.value = !(await exists(p));
    } catch {
      pathMissing.value = false;
    }
  },
  { immediate: true },
);
```

`Sidebar` 增加 `pathMissing` prop 透传给 `WorkspaceSwitcher`。

- [ ] **Step 4: 运行测试与 typecheck**

Run: `npm test && npm run typecheck`
Expected: 全部 PASS / 退出码 0

- [ ] **Step 5: 提交**

```bash
git add src/composables/agentSettingsFields.ts src/components/ChatView.vue src/components/ChatInput.vue src/components/AgentSettings.vue src/components/WorkspaceSwitcher.vue src/types/index.ts src/composables/useAgentSettings.test.ts src/App.vue src/components/Sidebar.vue
git commit -m "feat(workspace): 归档空间只读，Agent 设置按空间作用域"
```

---

### Task 12: 收尾 —— 文档更新与全量验收

**Files:**
- Modify: `docs/design-spec.md`
- Modify: `AGENTS.md`
- Modify: `README.md`

**Interfaces:** 无（文档与验收）

- [ ] **Step 1: 更新设计文档**

在 `docs/design-spec.md` 的"界面布局"小节补充工作空间切换器；"侧边栏组件"表新增"工作空间切换器"与"空间管理"两行；"系统架构"数据流补充"会话按 workspace_id 过滤"。

- [ ] **Step 2: 更新 AGENTS.md 与 README**

- `AGENTS.md`："常用命令"不变；"核心 Owner"补充 `src/composables/useWorkspaces.ts`；若新增脚本需同步。
- `README.md`：验收清单追加 `cd src-tauri && cargo test`；关键路径冒烟补充：切换工作空间、归档/恢复、新建空间复制设置、彻底删除二次确认。

- [ ] **Step 3: 全量验收**

```bash
npm test
npm run typecheck
npm run build
cd src-tauri && cargo test
```

Expected: 全部退出码 0。

手动冒烟（`npm run tauri dev`）：
1. 旧数据启动 → 出现"默认工作空间"，旧会话/设置完整；
2. 切换空间 → 会话列表过滤、空间名/路径/色点更新；
3. 新建空间（复制设置）→ 设置与 MCP 独立；
4. Agent 运行中切换器禁用；
5. 归档空间只读横幅、禁止新建/发送；恢复后可对话；
6. 彻底删除需输入空间名确认；
7. `npm run dev` 浏览器模式无崩溃。

- [ ] **Step 4: 提交**

```bash
git add docs/design-spec.md AGENTS.md README.md
git commit -m "docs(workspace): 更新设计文档与验收清单"
```

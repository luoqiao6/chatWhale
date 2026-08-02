# chatWhale 工作空间管理设计

日期：2026-08-02
状态：待审阅

## 1. 背景与目标

chatWhale 当前把 Agent 的工作目录（`agent.workspace_root`）当作唯一的"工作空间"：它是一个藏在 Agent 设置弹窗里的全局字符串，切换项目需要反复打开设置、改路径、保存；同时会话存储在一个全局列表中，与工作空间没有任何关联，无法按项目组织历史对话。

本次改造的目标：

1. 将"工作空间"升级为**完整工作区**：一个空间 = 一个工作目录 + 一套独立的 Agent 设置与 MCP 服务器 + 归属于该空间的会话历史；
2. 提供**一键切换**工作空间的入口，并让当前空间在界面上始终可见；
3. 让**会话与空间的关系可见**：会话归属空间、按空间过滤展示、带有空间标识；
4. 兼容现有用户数据：首次升级自动迁移，不丢会话、不丢设置。

## 2. 已确认决策

与用户逐项确认的决策如下：

| # | 决策项 | 结论 |
|---|--------|------|
| 1 | 工作空间模型 | 完整工作区：目录 + 独立 Agent 设置 + MCP + 会话历史 |
| 2 | 旧数据迁移 | 首次升级自动创建"默认工作空间"，现有会话/设置/MCP 全部归入其中；新会话归属创建时所在空间 |
| 3 | 设置作用域 | `agent.*` 全部设置与 MCP Server 按空间彻底隔离；API Key、主题、模型保持全局；新建空间可复制设置 |
| 4 | 空间删除 | 软删除/归档：会话保留，空间不可继续对话，可恢复或彻底清除 |
| 5 | UI 方案 | 侧边栏顶部空间切换器（方案 1），会话列表按当前空间过滤 |

## 3. 现状梳理

### 3.1 存储双轨

- **前端会话（实际主链路）**：`src/composables/useConversations.ts` 将全部会话读写 localStorage 键 `chatwhale-conversations`；`ChatView.vue` 通过它读取/保存消息。
- **Rust SQLite（`~/.chatwhale/chatwhale.db`）**：已有 `conversations`、`agent_settings`、`mcp_servers`、`settings` 四张表，`src-tauri/src/db.rs` 提供完整 CRUD，但前端主链路未使用 `conversations` 的 Rust 命令（`src/composables/useChat.ts` 中残留了 invoke 调用但未被 App 使用）。
- **Agent 设置与 MCP**：`AgentSettings.vue` 通过 `get_agent_settings` / `set_agent_settings` / `list_mcp_servers` 等命令读写 SQLite，键值对与 MCP 表均为全局作用域。

### 3.2 现有表结构

```sql
conversations(id TEXT PK, title, model, created_at, updated_at, messages)
agent_settings(key TEXT PK, value)
mcp_servers(id TEXT PK, name, command, args, env, cwd, timeout, transport, enabled, created_at, updated_at)
settings(key TEXT PK, value)          -- 预留，本次不使用
```

### 3.3 现有设置键（`AGENT_SETTING_KEYS`）

`agent.workspace_root`、`agent.skills_dir`、`agent.command_approval`、`agent.max_iterations`、`agent.llm_timeout`、`agent.command_timeout`、`agent.approval_timeout`、`agent.max_result_bytes`、`agent.command_whitelist`、`agent.sensitive_paths`。

### 3.4 现状问题

- 工作目录是全局唯一的字符串，切换需进入设置弹窗；
- 会话无 `workspace_id`，无法按空间组织；
- 会话存储双轨（localStorage 与 SQLite）并存，为归属空间埋下不一致隐患；
- 界面任何位置都不显示当前空间，空间与会话关系不可见。

## 4. 数据模型设计

### 4.1 新增 `workspaces` 表

```sql
CREATE TABLE IF NOT EXISTS workspaces (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    path       TEXT NOT NULL DEFAULT '',
    archived   INTEGER NOT NULL DEFAULT 0,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
```

约束与约定：

- `id = "default"` 的默认工作空间由迁移创建，**不可归档、不可删除**，可重命名；
- `path` 为绝对路径，可为空字符串（等价于未配置目录，文件类工具保持"请先配置工作目录"的语义）；
- `archived`：`1` 表示已归档。

### 4.2 改造 `conversations` 表

新增列：

```sql
ALTER TABLE conversations ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
CREATE INDEX IF NOT EXISTS idx_conversations_workspace ON conversations(workspace_id);
```

- 所有会话必须归属一个空间；查询按 `workspace_id` 过滤；
- `Conversation` 前端类型与 Rust 结构体同步增加 `workspace_id` 字段。

### 4.3 改造 `agent_settings` 表

作用域化：由单一 `key` 主键改为 `(workspace_id, key)` 复合主键。

```sql
-- 旧表存在时：新建带作用域的新表，复制旧数据到 'default'，再替换
ALTER TABLE agent_settings RENAME TO agent_settings_legacy;
CREATE TABLE agent_settings (
    workspace_id TEXT NOT NULL DEFAULT 'default',
    key          TEXT NOT NULL,
    value        TEXT NOT NULL,
    PRIMARY KEY (workspace_id, key)
);
INSERT INTO agent_settings (workspace_id, key, value)
    SELECT 'default', key, value FROM agent_settings_legacy;
DROP TABLE agent_settings_legacy;
```

- 换表方式比 `ALTER TABLE` 加列更干净（主键结构变化无法用加列实现）；
- `AGENT_SETTING_KEYS` 的默认值种子改为按 `(workspace_id, key)` 写入，仅在目标空间缺键时补默认值。

### 4.4 改造 `mcp_servers` 表

新增列：

```sql
ALTER TABLE mcp_servers ADD COLUMN workspace_id TEXT NOT NULL DEFAULT 'default';
CREATE INDEX IF NOT EXISTS idx_mcp_servers_workspace ON mcp_servers(workspace_id);
```

所有 MCP 查询（列表、启用的服务器）均带 `workspace_id` 过滤。

### 4.5 表结构迁移（Rust，`db.rs`）

迁移在 `Database::new()` 中执行，顺序固定：

1. 用 `PRAGMA table_info` 检测 `workspaces` 是否存在；不存在则建表；
2. 检测 `conversations` / `mcp_servers` 是否缺少 `workspace_id` 列，缺少则 `ALTER TABLE` 加列并建索引；
3. 检测 `agent_settings` 是否仍为旧结构（主键为 `key`），是则按 4.3 的换表流程迁移；
4. 数据迁移：若 `workspaces` 表为空，创建默认工作空间：
   - `id = "default"`，`name = "默认工作空间"`，`path = 旧 agent.workspace_root 值`（可能为空），`archived = 0`；
   - 旧会话、旧设置、旧 MCP 已通过 DEFAULT 'default' 或换表归位，无需逐行 UPDATE；
5. 更新 `updated_at` 为迁移时间。

迁移为幂等操作：已是最新结构时全部跳过。

### 4.6 前端 localStorage 迁移（浏览器模式降级）

浏览器模式（无 Tauri）下会话与空间信息存 localStorage：

- 读取 `chatwhale-conversations` 时，若会话对象缺少 `workspace_id`，视为 `"default"` 并回写；
- 新增 `chatwhale-workspaces` 键保存空间列表（含默认空间）；
- `chatwhale-active-workspace` 保存当前空间 id；
- 首次读取（无 `chatwhale-workspaces`）时创建默认空间，并把旧设置入口标记为"仅 Tauri 模式可用"。

## 5. 架构与组件

### 5.1 Rust 后端（`src-tauri/src`）

新增/修改的 Tauri 命令：

| 命令 | 说明 |
|------|------|
| `list_workspaces` | 返回全部空间（含归档），按 `created_at` 升序；每项附带 `conversation_count`（`COUNT` 统计），供切换器展示会话数 |
| `create_workspace { name, path, copy_from }` | 新建空间；`copy_from` 为空间 id 或 `null`，非空时复制该空间的 `agent_settings` 与 `mcp_servers`（MCP 复制时重新生成 id，其余字段原样）；与默认空间 id 冲突时报错 |
| `update_workspace { id, name?, path? }` | 重命名/改路径；`id = "default"` 仍可重命名 |
| `set_workspace_archived { id, archived }` | 归档/恢复；`id = "default"` 时拒绝归档 |
| `delete_workspace { id }` | 彻底删除：删空间行 + 该空间会话 + 设置 + MCP；`id = "default"` 拒绝 |
| `get_agent_settings { workspace_id }` | 改为按空间读取 |
| `set_agent_settings { workspace_id, settings }` | 按空间写入 |
| `list_mcp_servers { workspace_id }` / `add/update/remove_mcp_server` | 全部加 `workspace_id` 参数 |
| `get_conversations { workspace_id }` | 按空间过滤（保留无参默认 `"default"`，兼容旧调用） |
| `create_conversation { title, model, workspace_id }` / `update_conversation` / `delete_conversation` | 会话绑定/过滤空间 |
| `agent_chat` | 参数增加 `workspace_id`，加载该空间的 `AgentSettings` 后运行 |

运行期约束（`src-tauri/src/agent/mod.rs`）：

- `run_agent` 前检查目标空间 `archived = 0`，已归档则返回错误"该工作空间已归档，请先恢复后再继续对话"；
- `AgentSettings::load` 改为按 `workspace_id` 加载设置，`AGENT.md`、Skills 扫描、路径沙箱行为不变（读取的是当前空间的 `workspace_root` 与 `skills_dir`）；
- 文件工具错误文案保持现状（未配置目录时"请先在 Agent 设置中配置工作目录"）。

**工作目录唯一来源**：`workspaces.path` 是工作目录的唯一权威来源。`agent.workspace_root` 设置键退役：不再在 Agent 设置表单中展示，`AgentSettings::load` 的 `workspace_root` 改从当前空间记录读取（迁移时旧值已写入 `workspaces.path`），避免双源不一致。

### 5.2 前端 composables

**新增 `src/composables/useWorkspaces.ts`**（单例状态）：

```ts
interface Workspace {
  id: string;
  name: string;
  path: string;
  archived: boolean;
  created_at: number;
  updated_at: number;
}
```

- `workspaces` / `currentWorkspace` / `activeWorkspaces` / `archivedWorkspaces`；
- `initWorkspaces()`：启动时加载；Tauri 模式调 `list_workspaces`，浏览器模式读 localStorage；
- `switchWorkspace(id)`：保存当前会话 → 切换 `currentWorkspace` → 触发会话列表重载；
- `createWorkspace({ name, path, copyFrom })`、`renameWorkspace`、`setArchived`、`deleteWorkspace`；
- `colorOf(id)`：用固定调色板按 id 哈希取色，供空间标识使用。

**修改 `src/composables/useConversations.ts`**：

- 数据源统一：Tauri 模式下走 Rust 命令（`get_conversations`、`create_conversation`、`update_conversation`、`delete_conversation`），浏览器模式回退 localStorage；
- `Conversation` 增加 `workspace_id`；`createConversation` 自动绑定 `currentWorkspace.id`；
- `groupedConversations` 仅聚合当前空间会话（按 `workspace_id` 过滤后再按时间分组）；
- `moveConversation(id, targetWorkspaceId)`：把会话移动到目标活跃空间（供归档逃生口与整理用）；
- 删除旧的 localStorage 写路径（`chatwhale-conversations` 仅作为浏览器模式降级存储）。

**修改 `src/composables/useAgent.ts`**：`startAgent` 传递 `workspaceId: currentWorkspace.id`。

### 5.3 前端组件

**新增 `src/components/WorkspaceSwitcher.vue`**（侧边栏顶部，品牌区下方）：

- 收起态：空间色点 + 空间名（粗体）+ 路径（省略号截断）+ ▾ 箭头；显示当前空间；
- 点击展开弹出层：
  - "工作空间"分组：活跃空间列表（名称、路径、会话数）；
  - "已归档"分组：归档空间列表（只读标识），可展开恢复操作；
  - 底部按钮："+ 新建工作空间"、"管理空间…"；
  - 点击空间项即切换；点击外部/按 Esc 关闭；
- 弹出层最大高度受限，内部滚动；空间数量多时不影响侧边栏布局；
- Agent 运行中（`isAgentRunning`）时切换器整体禁用，title 提示"Agent 正在运行，请稍后再切换"。

**新增 `src/components/WorkspaceManager.vue`**（设置级弹窗）：

- 新建空间表单：目录选择（Tauri `plugin-dialog` 选目录，浏览器模式手输路径）+ 名称（默认取目录名，可改）+ 复制来源下拉（"不复制" / "默认工作空间" / "当前空间"）；
- 空间列表：名称、路径、会话数、归档状态；操作：重命名、打开该空间的 Agent 设置（复用 `AgentSettings.vue`，传入目标空间 id）、归档/恢复、彻底删除；
- 彻底删除流程：二次确认弹窗，明确提示"将永久删除该空间的 N 个会话及其设置，且不可恢复"，输入空间名确认后执行；
- 默认空间：不显示归档/彻底删除按钮，可重命名。

**修改 `src/components/Sidebar.vue`**：

- 品牌区下方插入 `<WorkspaceSwitcher />`；
- 会话列表空态文案改为"该工作空间暂无对话"；
- 会话项左侧增加空间色点（当前空间内同色，用于强化归属感）；保留时间分组标题；
- 会话项悬停出现"更多"菜单：移动到其他空间、删除（复用现有删除逻辑）。

**修改 `src/components/ChatView.vue`**：

- 当前空间已归档时：输入区禁用，顶部显示横幅"此工作空间已归档，可查看历史会话；继续对话请恢复工作空间"；
- 切换空间触发 `loadMessagesFromConv` 重载（通过 `currentConvId` 变化 + 空间变化双重 key 失效）。

**修改 `src/components/AgentSettings.vue`**：

- 所有读写命令带 `workspace_id`；
- 标题显示当前作用域（"Agent 设置 · 项目A"）；
- 保存逻辑不变，新增 `workspace_id` 参数。

**修改 `src/App.vue`**：

- `onMounted` 先 `initWorkspaces()`，再加载会话列表；
- `currentConvId` 在切换空间后若不属于新空间则置 `null`；
- 透传当前空间给 `Sidebar` / `ChatView` / `AgentSettings`。

### 5.4 数据流与切换时序

切换空间（`switchWorkspace`）：

1. 若 Agent 运行中：拒绝切换（按钮禁用）；
2. 若存在未保存消息且 `currentConvId` 有效：先保存到当前空间；
3. 写入 `currentWorkspace.id`：统一持久化到 localStorage `chatwhale-active-workspace`（应用偏好；空间列表本身在 Tauri 模式存 SQLite、浏览器模式存 localStorage）；
4. 重新拉取目标空间会话列表；
5. 若 `currentConvId` 不属于目标空间，置 `null`，聊天区显示空态；
6. 渲染切换到目标空间（侧边栏列表、色点、AgentSettings 作用域同步更新）。

新建会话：`createConversation` 传入当前空间 id，会话列表立即显示。

## 6. 交互设计细节（方案 1）

### 6.1 侧边栏布局

```
┌────────────────────────┐
│ 🐋 chatWhale     ⚙ ⚙   │  品牌区（不变）
├────────────────────────┤
│ ▣ 项目A          ▾     │  空间切换器（新增）
│    /Users/me/work/A    │
├────────────────────────┤
│ [+ 新建对话]            │
│ 今天                   │
│ ● 会话1                │
│ ● 会话2                │
│ 本周                   │
│ ● 会话3                │
├────────────────────────┤
│ 界面主题 …              │  底部（不变）
│ 模型选择器 …            │
└────────────────────────┘
```

### 6.2 空间标识

- 调色板固定 8 色（与主题无关，半透明背景便于深浅主题通用）；
- `colorOf(id)`：`id` 的简单哈希对 8 取模；默认空间固定为鲸青色（`--accent` 同源色）；
- 色点出现在：切换器当前项、弹出层空间项、会话列表项。

### 6.3 状态与空态

| 场景 | 表现 |
|------|------|
| 空间无会话 | "该工作空间暂无对话" |
| 空间已归档 | 切换器该项带归档角标；聊天区只读横幅；新建对话按钮禁用并提示"已归档" |
| 未配置工作目录（path 为空） | 切换器路径显示"未配置目录"，文件工具保持现有禁用语义；新建空间表单中路径为空允许保存 |
| 目录已被删除/不可达 | 切换时正常加载，路径处显示警示样式；Agent 运行遇路径错误沿用现有错误提示，不额外阻断 |

## 7. 错误处理与边界

- **默认空间保护**：`id = "default"` 不可归档、不可删除；数据库层再次校验（`update_workspace` / `delete_workspace` 返回错误）。
- **归档空间禁写**：`run_agent`、`create_conversation`、`set_agent_settings`（对归档空间）返回明确错误；前端按钮同步禁用。
- **复制设置来源缺失**：`copy_from` 指向不存在的空间时忽略复制并新建空设置空间（默认值兜底），不报错中断。
- **并发/运行中切换**：前端通过 `isAgentRunning` 禁用；Rust 侧不强制（单窗口应用，前端约束足够）。
- **浏览器模式降级**：空间与会话存 localStorage；Agent 设置/MCP 相关命令失败时沿用现有 `errorMsg` 提示，不崩溃。
- **安全边界不回归**：API Key 仍在 localStorage（`chatwhale-api-key`），不进入 SQLite、不进日志、不进源码；`v-html` 渲染模型内容的净化边界不变（本次不引入新的 HTML 注入点）。
- **数据删除**：`delete_workspace` 为物理删除且不可恢复，前端需输入空间名二次确认；会话无独立回收站（归档即回收站语义）。

## 8. 测试计划

### 8.1 Rust（`cargo test`，`src-tauri/tests` + `db.rs` 单元测试）

- **迁移测试**：
  - 空库 → 创建 `workspaces` 表与默认空间；
  - 旧结构库（无 `workspace_id` 列、`agent_settings` 旧主键）→ 迁移后列存在、数据全部归 `"default"`；
  - 重复执行迁移幂等。
- **workspace CRUD**：创建（含 `copy_from` 复制设置/MCP）、重命名、归档/恢复、彻底删除（级联删会话/设置/MCP）；
- **默认空间保护**：归档/删除默认空间被拒绝；
- **作用域隔离**：两个空间的 `agent_settings`、`mcp_servers`、`conversations` 互不可见；
- **归档空间运行约束**：`run_agent` 对归档空间返回错误（可通过命令层单元测试验证设置加载分支）。

### 8.2 前端（`npm test`，vitest）

- `useWorkspaces`：初始化（Tauri/localStorage 分支）、切换、创建、归档/恢复、删除、`colorOf` 稳定性；
- `useConversations`：按空间过滤分组；创建会话绑定当前空间；旧 localStorage 数据（无 `workspace_id`）迁移到 `"default"`；`moveConversation`；
- `WorkspaceSwitcher` / `WorkspaceManager`：用现有测试工具（vitest + @vue/test-utils，若项目未引入则先按现有测试风格补充）覆盖渲染与关键交互（切换回调、归档禁用、二次确认）。

### 8.3 验收清单

```bash
npm test
npm run typecheck
npm run build        # 内置 typecheck + vitest + vite build
cd src-tauri && cargo test
```

### 8.4 手动验收（Tauri 模式）

1. 旧版本数据启动 → 出现"默认工作空间"，旧会话与设置完整可见；
2. 新建空间（选目录 + 复制设置）→ 设置与 MCP 独立，修改后不影响其他空间；
3. 切换空间 → 会话列表立即过滤，当前空间名/路径/色点更新；
4. Agent 运行中切换器禁用；
5. 归档空间 → 只读横幅、禁止新建/发送；恢复后可正常对话；
6. 彻底删除 → 二次确认（输入空间名）后数据消失；
7. `npm run dev` 浏览器模式 → 空间与会话降级可用，无崩溃。

## 9. 明确不做（YAGNI）

- 空间内会话搜索/过滤（当前版本侧边栏本无搜索，不扩大范围）；
- 会话拖拽迁移（提供"移动到空间"菜单替代）；
- 空间级主题（主题保持全局）；
- 多窗口/多标签；
- 会话导出/导入（彻底删除前建议用户先恢复空间，导出列为后续）。

## 10. 实施阶段建议（供 writing-plans 拆分）

| 阶段 | 内容 | 依赖 |
|------|------|------|
| P1 | Rust 数据模型：workspaces 表、表结构迁移、数据迁移、workspace CRUD 命令、现有命令加 `workspace_id` | 无 |
| P2 | Rust 运行期：`agent_chat` 按空间加载设置、归档空间拒绝运行、`copy_from` 复制逻辑 | P1 |
| P3 | 前端数据层：`useWorkspaces`、`useConversations` 统一数据源 + 空间过滤、`useAgent` 传参 | P1 |
| P4 | 前端界面：`WorkspaceSwitcher`、`WorkspaceManager`、Sidebar/ChatView/AgentSettings 集成 | P3 |
| P5 | 测试补齐、文档更新（AGENTS.md、README 验收清单、docs/design-spec.md）、全量验收 | P1–P4 |

## 11. 影响的现有文件

- 新增：`src-tauri/src/workspace.rs`（命令与迁移逻辑，或并入 `db.rs`，实现时按既有模块风格决定）、`src/composables/useWorkspaces.ts`、`src/components/WorkspaceSwitcher.vue`、`src/components/WorkspaceManager.vue`；
- 修改：`src-tauri/src/db.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/agent/mod.rs`、`src-tauri/src/agent/types.rs`、`src/composables/useConversations.ts`、`src/composables/useAgent.ts`、`src/types/index.ts`、`src/App.vue`、`src/components/Sidebar.vue`、`src/components/ChatView.vue`、`src/components/AgentSettings.vue`；
- 文档：`docs/design-spec.md`、`AGENTS.md`、`README.md`（验收清单与功能说明）。

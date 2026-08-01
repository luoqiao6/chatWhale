<!--
  chatWhale Agent 能力设计方案
  版本: 1.0
  日期: 2026-08-01
  状态: 待实现
-->

# chatWhale Agent 能力设计方案

## 1. 概述

在 chatWhale 现有聊天能力之上，增加 Agent 模式，使客户端具备：

- **工具调用回路** — LLM 返回 tool_calls 后自动执行工具并将结果传回，形成完整的调用循环
- **内置 Agent 工具** — 文件读写、目录浏览、命令执行、文件搜索等本地能力
- **Skills 系统** — 加载 SKILL.md 文件，动态扩展 Agent 的指令和工具集
- **AGENT.md 支持** — 自动读取项目根目录的 AGENT.md，注入项目级指令到 system prompt
- **MCP 集成** — 连接外部 MCP Server，发现和调用第三方工具

## 2. 架构决策

Agent 的工具执行回路放在 **Rust 后端**，原因：

- Rust 端可以安全地读写文件、执行 shell 命令
- 利用 Tauri 的安全沙箱模型做权限控制
- 工具调用回路不依赖前端生命周期
- tokio 异步运行时天然适合处理并发的 SSE 流 + 工具执行

前端（Vue 3）仅负责：

- 发送 agent_chat 指令
- 监听 Tauri Events 实时渲染 Agent 的执行过程
- 展示工具调用卡片、MCP 来源标记、最终回答

## 3. 模块结构

```
src-tauri/src/
├── main.rs
├── lib.rs                    # 新增 agent 相关 Tauri commands + events
├── db.rs                     # 扩展: mcp_servers 表, agent_settings 表
├── sse.rs                    # 保留不修改
└── agent/
    ├── mod.rs                # Agent 编排器: 工具执行回路
    ├── tools.rs              # Tool trait + 注册表 + 内置工具实现
    ├── skills.rs             # SKILL.md 解析加载
    ├── agent_config.rs       # AGENT.md 读取
    └── mcp/
        ├── mod.rs            # MCP 客户端管理器
        ├── transport.rs      # stdio / SSE 传输层
        └── types.rs          # MCP 协议 JSON-RPC 类型定义
```

## 4. 核心数据流

```
前端 ChatView                       Rust agent::mod
  │                                      │
  │── invoke("agent_chat", {            │
  │     messages, model, base_url,      │
  │     api_key, temperature, ...       │
  │   }) ─────────────────────────────▶ │
  │                                      │── 加载 AGENT.md → system prompt 片段
  │                                      │── 扫描 Skills 目录 → 指令 + 工具定义
  │                                      │── 启动配置的 MCP Servers
  │                                      │── tools/list → 收集 MCP 工具
  │                                      │── 合并 tools 列表 (内置 + Skills + MCP)
  │                                      │
  │                                      │── POST /chat/completions ──▶ LLM API
  │   ◀── event: agent-chunk ──────── │   ◀── SSE stream (逐 token)
  │   ◀── event: agent-reasoning ──── │
  │                                      │
  │                                      │   ◀── finish_reason = tool_calls
  │                                      │── 判断工具来源:
  │                                      │   ├─ 内置工具 → 本地执行
  │                                      │   └─ MCP 工具  → 转发 MCP Server
  │   ◀── event: agent-tool-start ─── │
  │                                      │── 执行完成
  │   ◀── event: agent-tool-result ── │
  │                                      │── 结果塞回 messages
  │                                      │── 再次 POST /chat/completions
  │                                      │      ...循环直到 finish_reason = stop
  │   ◀── event: agent-done ────────── │
```

### 4.1 工具执行回路伪代码

```rust
// agent/mod.rs 核心逻辑
async fn run_agent(
    app_handle: &AppHandle,
    messages: &mut Vec<Message>,
    tools: &[ToolDef],
    tool_registry: &ToolRegistry,
    mcp_manager: &McpManager,
    config: &AgentConfig,
) -> Result<()> {
    let max_iterations = 10; // 防止无限循环

    for iteration in 0..max_iterations {
        let response = send_chat_request(&config, messages, tools).await?;
        let choice = &response.choices[0];

        match choice.finish_reason.as_deref() {
            Some("stop") => {
                // Agent 正常结束
                app_handle.emit("agent-done", &messages)?;
                return Ok(());
            }
            Some("tool_calls") => {
                for tool_call in &choice.message.tool_calls {
                    app_handle.emit("agent-tool-start", tool_call)?;

                    let result = if is_mcp_tool(&tool_call.function.name) {
                        mcp_manager.call_tool(&tool_call).await?
                    } else {
                        tool_registry.execute(&tool_call).await?
                    };

                    app_handle.emit("agent-tool-result", &result)?;

                    // 追加 assistant 消息 (含 tool_calls) 和 tool 结果消息
                    messages.push(assistant_message_with_tool_calls(tool_call));
                    messages.push(tool_result_message(tool_call.id, result));
                }
            }
            _ => { /* 处理其他 finish_reason */ }
        }
    }

    // 超过最大迭代次数
    app_handle.emit("agent-error", "超过最大工具调用次数")?;
    Ok(())
}
```

## 5. Tauri Events 定义

| Event | 方向 | Payload | 触发时机 |
|-------|------|---------|---------|
| `agent-chunk` | Rust → Vue | `{ content: string }` | LLM 返回内容 token |
| `agent-reasoning` | Rust → Vue | `{ content: string }` | LLM 返回推理 token |
| `agent-tool-start` | Rust → Vue | `{ id, name, arguments, source }` | 开始执行工具 |
| `agent-tool-result` | Rust → Vue | `{ id, name, result, error? }` | 工具执行完成 |
| `agent-done` | Rust → Vue | `{ messages: Message[] }` | Agent 循环结束 |
| `agent-error` | Rust → Vue | `{ message: string }` | 任何阶段出错 |

## 6. 各模块详细设计

### 6.1 agent/tools.rs — 内置工具系统

#### Tool trait

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> serde_json::Value;  // JSON Schema
    fn execute(&self, arguments: serde_json::Value) -> Result<ToolResult>;
}

pub struct ToolResult {
    pub success: bool,
    pub content: String,
}
```

#### ToolRegistry — 工具注册表

```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { /* ... */ }
    pub fn register(&mut self, tool: Box<dyn Tool>) { /* ... */ }
    pub fn get(&self, name: &str) -> Option<&dyn Tool> { /* ... */ }
    pub fn list_definitions(&self) -> Vec<ToolDef> { /* ... */ }
    pub async fn execute(&self, tool_call: &ToolCall) -> Result<ToolResult> { /* ... */ }
}
```

#### 首批内置工具

| 工具名 | 参数 | 功能 | 安全约束 |
|--------|------|------|---------|
| `read_file` | `{ path: string }` | 读取文件内容 | 限制在 workspace 目录内 |
| `write_file` | `{ path: string, content: string }` | 写入文件 | 限制在 workspace 目录内 |
| `list_directory` | `{ path: string }` | 列出目录内容 | 限制在 workspace 目录内 |
| `search_files` | `{ pattern: string, path: string }` | 递归搜索文件内容 | 限制在 workspace 目录内 |
| `execute_command` | `{ command: string, cwd?: string }` | 执行 shell 命令 | 需用户审批 / 白名单 |

#### 安全模型

- **Workspace 沙箱**：文件类工具默认限制在用户配置的 workspace 根目录内
- **命令执行审批**：execute_command 默认需要前端用户确认，或支持命令白名单
- **只读工具**：read_file、list_directory、search_files 不修改文件系统

### 6.2 agent/skills.rs — Skills 系统

#### 目录结构

```
~/.chatwhale/skills/             # 全局 Skills 目录
├── my-skill/
│   └── SKILL.md                 # 技能定义文件
├── code-review/
│   └── SKILL.md
└── ...
```

以及项目本地目录（AGENT.md 所在目录下的 `.skills/`）。

#### SKILL.md 格式

```markdown
---
name: code-review
description: 代码审查技能，用于在提交前检查代码质量
triggers:
  - "帮我审查"
  - "review 代码"
---

# Code Review Skill

当用户请求代码审查时，你会：

1. 检查代码风格和命名规范
2. 扫描潜在 bug 和安全问题
3. 提出改进建议
```

#### 解析逻辑

```rust
pub struct Skill {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub instructions: String,  // SKILL.md 正文内容
    pub tools: Vec<ToolDef>,   // 技能定义的额外工具 (如有)
}

pub struct SkillManager {
    skills_dir: PathBuf,
    project_skills_dir: Option<PathBuf>,
    loaded_skills: Vec<Skill>,
}

impl SkillManager {
    pub fn new() -> Self { /* ... */ }
    pub fn load_all(&mut self, workspace_root: Option<&Path>) -> Result<()> { /* ... */ }
    pub fn matching_skills(&self, user_message: &str) -> Vec<&Skill> { /* ... */ }
    pub fn system_prompt_fragment(&self) -> String { /* ... */ }
}
```

### 6.3 agent/agent_config.rs — AGENT.md 支持

#### 查找策略

1. 用户配置的 workspace 根目录 / AGENT.md
2. ~/.chatwhale/AGENT.md（全局默认）

#### 解析

```rust
pub struct AgentConfig {
    pub workspace_root: Option<PathBuf>,
    pub agent_md_content: Option<String>,
    pub skills_dir: Option<PathBuf>,
}

impl AgentConfig {
    pub fn load(workspace_root: Option<&Path>) -> Self { /* ... */ }
    pub fn system_prompt_base(&self) -> String { /* ... */ }
}
```

AGENT.md 内容直接注入到 system prompt 开头，允许用户定义项目级别的 Agent 行为约束。

### 6.4 agent/mcp/ — MCP 集成

#### 协议支持

- JSON-RPC 2.0 over stdio transport（一期）
- SSE transport（后续）

#### Cargo 依赖

```toml
rmcp = { version = "0.4", features = ["client", "transport-child-process"] }
```

#### 核心结构

```rust
// mcp/types.rs
pub struct McpServerConfig {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub enabled: bool,
}

// mcp/transport.rs
pub struct McpTransport { /* stdio child process management */ }

// mcp/mod.rs
pub struct McpManager {
    servers: HashMap<String, McpServerState>,
}

struct McpServerState {
    config: McpServerConfig,
    transport: McpTransport,
    tools: Vec<ToolDef>,
}

impl McpManager {
    pub async fn connect(server: &McpServerConfig) -> Result<McpServerState> { /* ... */ }
    pub async fn list_tools(&mut self, server_id: &str) -> Result<Vec<ToolDef>> { /* ... */ }
    pub async fn call_tool(&mut self, server_id: &str, tool_call: &ToolCall) -> Result<ToolResult> { /* ... */ }
    pub fn all_tools(&self) -> Vec<ToolDef> { /* ... */ }
    pub fn shutdown_all(&mut self) { /* ... */ }
}
```

#### 生命周期

```
Agent 启动
  └─ 从 SQLite 读取 mcp_servers WHERE enabled = 1
  └─ 逐个 spawn 子进程
  └─ 发送 initialize 请求
  └─ 发送 tools/list 请求
  └─ 收集所有 MCP 工具 → 合并到 agent tools 列表

Agent 运行中
  └─ LLM 调用 MCP 工具 → McpManager.call_tool()
  └─ 串行化 JSON-RPC tools/call 请求
  └─ 等待响应 → 返回 ToolResult

Agent 结束 / 取消
  └─ 发送 shutdown 通知给所有 MCP Server
  └─ 关闭子进程
```

## 7. 数据库变更

```sql
-- MCP Server 配置
CREATE TABLE IF NOT EXISTS mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    command TEXT NOT NULL,
    args TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

-- Agent 全局设置
CREATE TABLE IF NOT EXISTS agent_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- 预置 agent 设置 key:
--   agent.workspace_root      - Agent 工作目录
--   agent.max_iterations       - 工具调用最大循环次数 (默认 10)
--   agent.skills_dir           - 全局 Skills 目录
--   agent.command_approval     - 命令执行审批策略: always | whitelist | never
```

## 8. 前端变更

### 8.1 新增 Composable: useAgent.ts

```typescript
// src/composables/useAgent.ts
export function useAgent() {
  const agentMessages = ref<Message[]>([]);
  const isAgentRunning = ref(false);
  const currentToolCall = ref<ToolExecution | null>(null);

  async function startAgent(params: AgentChatParams) {
    // invoke("agent_chat", params)
    // 监听 agent-chunk / agent-reasoning / agent-tool-start
    //       / agent-tool-result / agent-done / agent-error
  }

  async function cancelAgent() {
    // invoke("agent_cancel")
  }

  return { agentMessages, isAgentRunning, currentToolCall, startAgent, cancelAgent };
}
```

### 8.2 ChatView 增强

- 输入区域增加 Agent 模式开关（tool + brain 图标切换）
- Agent 模式下的 handleSend 调用 useAgent().startAgent
- 工具调用期间 isLoading 持续直到 agent-done

### 8.3 MessageBubble 增强

- 工具调用卡片显示来源标记（内置 / MCP + server 名称）
- 工具结果可折叠展示
- 工具执行中旋转动画

### 8.4 新增 AgentSettings.vue

- Agent 模式开关
- Workspace 根目录选择
- MCP Server 管理（增删改查、启用/禁用）
- Skills 目录配置
- 命令执行审批策略

### 8.5 Sidebar 变更

- 增加 Agent 设置入口按钮

## 9. Tauri Commands

在 lib.rs 中新增：

| Command | 功能 |
|---------|------|
| `agent_chat` | 启动 Agent 对话 |
| `agent_cancel` | 取消正在运行的 Agent |
| `list_builtin_tools` | 列出内置工具 |
| `list_mcp_servers` | 列出 MCP Server 配置 |
| `add_mcp_server` | 添加 MCP Server |
| `remove_mcp_server` | 删除 MCP Server |
| `update_mcp_server` | 更新 MCP Server 启用状态 |
| `get_agent_settings` | 获取 Agent 设置 |
| `set_agent_settings` | 保存 Agent 设置 |

## 10. 错误处理策略

| 场景 | 处理方式 |
|------|---------|
| LLM API 调用失败 | agent-error event，展示错误信息 |
| 工具执行失败 | 返回 ToolResult { success: false }，以 tool role 告知 LLM |
| MCP Server 崩溃 | 重连一次，失败则移除工具并 agent-error |
| 超过最大迭代次数 | 停止循环，agent-done 返回当前 messages |
| 用户取消 | 终止循环，清理 MCP 子进程，agent-done |
| 读取超大文件 | 限制返回前 1000 行，截断并告知 LLM |

## 11. 实施顺序

| 阶段 | 内容 | 依赖 |
|------|------|------|
| Phase 1 | agent/mod.rs + tools.rs：工具执行回路 + 内置工具 | 无 |
| Phase 2 | 前端 useAgent.ts + ChatView 改造 + MessageBubble 增强 | Phase 1 |
| Phase 3 | agent_config.rs：AGENT.md 读取 + system prompt 注入 | Phase 1 |
| Phase 4 | skills.rs：Skills 目录扫描 + SKILL.md 解析 | Phase 1 |
| Phase 5 | mcp/ 模块：MCP Server 管理 + 工具发现调用 | Phase 1 |
| Phase 6 | AgentSettings.vue + 数据库扩展 | Phase 2-5 |
| Phase 7 | 安全审批流 + 权限控制 | Phase 1-5 |

## 12. 风险与注意事项

- **并发安全**：AppState 使用 Mutex\<Database\>，agent 运行期间注意避免长时间持锁
- **取消机制**：使用 tokio::select! 或 CancellationToken 实现可取消循环
- **MCP 兼容性**：不同 MCP Server 质量参差不齐，需要健壮的超时和错误处理
- **流式体验**：Agent 循环中 LLM 调用间的等待会打断流式感，工具执行期间显示进度指示
- **前端 SSE 现状**：当前 ChatView 直接 fetch 做 streaming，Agent 模式走 Rust 后端。考虑后续统一
